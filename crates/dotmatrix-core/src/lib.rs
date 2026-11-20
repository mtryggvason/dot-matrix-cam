//! Platform-independent dot-matrix effect.
//!
//! Pixels in, pixels out — no OS, camera or GPU code lives here, so this crate
//! compiles unchanged for native targets and for `wasm32-unknown-unknown`.

/// How the brightness of each cell is turned into a dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "lowercase"))]
pub enum Mode {
    /// Brighter cells get bigger dots (classic halftone).
    Size,
    /// Every dot has the same size; brightness controls how lit it is (LED panel look).
    Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct Params {
    /// Grid spacing in pixels. Smaller = more dots.
    pub cell: u32,
    /// Maximum dot diameter relative to the cell (0.0 – 1.5). 1.0 = dots just touch.
    pub dot_scale: f32,
    /// Dot colour (RGB).
    pub fg: [u8; 3],
    /// Background colour (RGB).
    pub bg: [u8; 3],
    pub mode: Mode,
    /// Contrast boost applied to cell brightness (1.0 = unchanged).
    pub contrast: f32,
    /// Use dark areas instead of bright areas to drive the dots.
    pub invert: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            cell: 12,
            dot_scale: 1.0,
            fg: [0x39, 0xff, 0x14],
            bg: [0, 0, 0],
            mode: Mode::Size,
            contrast: 1.2,
            invert: false,
        }
    }
}

/// Reusable scratch memory so rendering does not allocate per frame.
#[derive(Default)]
pub struct Renderer {
    cells: Vec<f32>,
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render `src` (packed pixels with `channels` = 3 (RGB) or 4 (RGBA) bytes per pixel)
    /// into `dst` as RGBA. `dst` must be `width * height * 4` bytes.
    pub fn render(
        &mut self,
        src: &[u8],
        channels: usize,
        width: u32,
        height: u32,
        p: &Params,
        dst: &mut [u8],
    ) {
        assert!(channels == 3 || channels == 4, "channels must be 3 or 4");
        let (w, h) = (width as usize, height as usize);
        assert!(src.len() >= w * h * channels, "src too small");
        assert!(dst.len() >= w * h * 4, "dst too small");
        if w == 0 || h == 0 {
            return;
        }

        let cell = p.cell.max(2) as usize;
        let cols = w.div_ceil(cell);
        let rows = h.div_ceil(cell);

        // 1. Average luminance per cell (0..1), with contrast and optional invert.
        self.cells.clear();
        self.cells.resize(cols * rows, 0.0);
        let mut counts = vec![0u32; cols * rows];
        for y in 0..h {
            let row = &src[y * w * channels..(y + 1) * w * channels];
            let cy = y / cell;
            for x in 0..w {
                let i = x * channels;
                // Rec. 709 luma, integer weights summing to 256.
                let l = (54 * row[i] as u32 + 183 * row[i + 1] as u32 + 19 * row[i + 2] as u32) >> 8;
                let c = cy * cols + x / cell;
                self.cells[c] += l as f32;
                counts[c] += 1;
            }
        }
        for (v, n) in self.cells.iter_mut().zip(&counts) {
            let mut l = *v / (*n as f32 * 255.0);
            l = ((l - 0.5) * p.contrast + 0.5).clamp(0.0, 1.0);
            *v = if p.invert { 1.0 - l } else { l };
        }

        // 2. Draw one anti-aliased circle per cell.
        let half = cell as f32 * 0.5;
        let max_r = half * p.dot_scale.clamp(0.0, 1.5);
        let fg = p.fg.map(|c| c as f32);
        let bg = p.bg.map(|c| c as f32);

        for y in 0..h {
            let cy = y / cell;
            let dy = (y - cy * cell) as f32 + 0.5 - half;
            let out = &mut dst[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                let cx = x / cell;
                let lum = self.cells[cy * cols + cx];
                let (r, intensity) = match p.mode {
                    // Dot area proportional to brightness → radius ∝ sqrt(brightness).
                    Mode::Size => (max_r * lum.sqrt(), 1.0),
                    Mode::Brightness => (max_r, lum),
                };
                let dx = (x - cx * cell) as f32 + 0.5 - half;
                let dist = (dx * dx + dy * dy).sqrt();
                let coverage = (r - dist + 0.5).clamp(0.0, 1.0) * intensity;
                let o = &mut out[x * 4..x * 4 + 4];
                o[0] = (bg[0] + (fg[0] - bg[0]) * coverage) as u8;
                o[1] = (bg[1] + (fg[1] - bg[1]) * coverage) as u8;
                o[2] = (bg[2] + (fg[2] - bg[2]) * coverage) as u8;
                o[3] = 255;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_input_draws_full_dot_center_and_bg_corner() {
        let (w, h) = (20u32, 20u32);
        let src = vec![255u8; (w * h * 4) as usize];
        let mut dst = vec![0u8; (w * h * 4) as usize];
        let p = Params { cell: 10, contrast: 1.0, fg: [255, 0, 0], bg: [0, 0, 255], ..Default::default() };
        Renderer::new().render(&src, 4, w, h, &p, &mut dst);
        let px = |x: u32, y: u32| &dst[((y * w + x) * 4) as usize..((y * w + x) * 4 + 3) as usize];
        assert_eq!(px(5, 5), &[255, 0, 0]); // cell center = dot colour
        assert_eq!(px(0, 0), &[0, 0, 255]); // cell corner = background
    }

    #[test]
    fn black_input_is_all_background() {
        let (w, h) = (16u32, 9u32);
        let src = vec![0u8; (w * h * 3) as usize];
        let mut dst = vec![0u8; (w * h * 4) as usize];
        let p = Params { contrast: 1.0, bg: [1, 2, 3], ..Default::default() };
        Renderer::new().render(&src, 3, w, h, &p, &mut dst);
        assert!(dst.chunks(4).all(|c| c == [1, 2, 3, 255]));
    }
}
