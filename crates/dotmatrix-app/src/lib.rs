//! Dot Matrix Cam app: one egui UI that runs as a native macOS window and,
//! compiled to WebAssembly, in the browser.
//!
//! Frame flow: camera (platform-specific) → `dotmatrix_core::Renderer` → egui texture.

pub mod ui;

#[cfg(not(target_arch = "wasm32"))]
mod camera_native;
#[cfg(target_arch = "wasm32")]
mod camera_web;
#[cfg(target_arch = "wasm32")]
mod web;

use dotmatrix_core::{Params, Renderer};
use eframe::egui;

/// Somewhere camera frames come from.
pub trait FrameSource {
    /// If there is a frame newer than the last one (or `force` is set), call `f` with
    /// `(width, height, bytes_per_pixel, pixels)`. Returns whether `f` was called.
    fn poll(&mut self, force: bool, f: &mut dyn FnMut(u32, u32, usize, &[u8])) -> bool;
    /// Human-readable state ("Running", an error, …).
    fn status(&self) -> String;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_camera() -> Box<dyn FrameSource> {
    Box::new(camera_native::NativeCamera::start())
}

/// The main window: controls on the left, live preview on the right.
pub struct DotMatrixApp {
    source: Box<dyn FrameSource>,
    settings: Params,
    /// Settings used for the current texture, to re-render when a control changes.
    rendered_with: Option<Params>,
    renderer: Renderer,
    out: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    /// Called whenever the user changes a setting (e.g. to save it).
    on_change: Box<dyn FnMut(&Params)>,
    /// Narrow screens only: whether the bottom controls shelf is expanded.
    shelf_open: bool,
}

/// Below this window width (in points), the controls move from the left sidebar
/// into a collapsible shelf at the bottom.
const NARROW_WIDTH: f32 = 700.0;

impl DotMatrixApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        source: Box<dyn FrameSource>,
        settings: Params,
        on_change: Box<dyn FnMut(&Params)>,
    ) -> Self {
        ui::apply_theme(&cc.egui_ctx);
        Self {
            source,
            settings,
            rendered_with: None,
            renderer: Renderer::new(),
            out: Vec::new(),
            texture: None,
            on_change,
            shelf_open: true,
        }
    }

    /// Pull the newest camera frame and run the effect on it.
    fn update_texture(&mut self, ctx: &egui::Context) {
        let force = self.rendered_with != Some(self.settings);
        let Self { source, settings, renderer, out, texture, .. } = self;
        let rendered = source.poll(force, &mut |w, h, channels, px| {
            out.resize((w * h * 4) as usize, 0);
            renderer.render(px, channels, w, h, settings, out);
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], out);
            match texture {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => *texture = Some(ctx.load_texture("camera", img, egui::TextureOptions::LINEAR)),
            }
        });
        if rendered {
            self.rendered_with = Some(self.settings);
        }
    }
}

impl DotMatrixApp {
    /// Desktop: fixed sidebar on the left, video fitted (letterboxed) in the rest.
    fn desktop_layout(&mut self, ui: &mut egui::Ui, status: &str) {
        egui::Panel::left("controls")
            .exact_size(280.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(ui::PANEL).inner_margin(20))
            .show(ui, |ui| {
                if ui::controls(ui, &mut self.settings, status) {
                    (self.on_change)(&self.settings);
                }
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui::BG).inner_margin(16))
            .show(ui, |ui| {
                if let Some(t) = &self.texture {
                    let size = t.size_vec2();
                    let scale = (ui.available_width() / size.x).min(ui.available_height() / size.y);
                    // Mirrored like a selfie view.
                    let img = egui::Image::new((t.id(), size * scale))
                        .uv(egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0)))
                        .corner_radius(8);
                    ui.centered_and_justified(|ui| ui.add(img));
                }
            });
    }

    /// Phone / narrow window: the video fills the whole screen, and the controls
    /// float over it in a shelf pinned to the bottom (like CSS `position: absolute; bottom: 0`).
    fn mobile_layout(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, status: &str) {
        // 1. Full-screen video, cropped to fill (like CSS `object-fit: cover`).
        let screen = ctx.viewport_rect();
        let painter = ui.painter();
        painter.rect_filled(screen, 0.0, ui::BG);
        if let Some(t) = &self.texture {
            painter.image(t.id(), screen, cover_uv(t.size_vec2(), screen.size()), egui::Color32::WHITE);
        }

        // 2. The shelf: an `Area` is egui's floating layer, positioned absolutely on top.
        let height = if self.shelf_open { (screen.height() * 0.5).max(160.0) } else { 56.0 };
        egui::Area::new(egui::Id::unique("controls_shelf"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(screen.left(), screen.bottom() - height))
            .show(ctx, |ui| {
                let margin = egui::Margin { left: 16, right: 16, top: 4, bottom: 12 };
                egui::Frame::new()
                    .fill(ui::SHELF)
                    .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
                    .inner_margin(margin)
                    .show(ui, |ui| {
                        ui.set_width(screen.width() - margin.sum().x);
                        ui.set_height(height - margin.sum().y);
                        if ui::shelf_handle(ui, self.shelf_open).clicked() {
                            self.shelf_open = !self.shelf_open;
                        }
                        if self.shelf_open {
                            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                                if ui::controls(ui, &mut self.settings, status) {
                                    (self.on_change)(&self.settings);
                                }
                            });
                        }
                    });
            });
    }
}

/// Texture coordinates that crop an image of size `tex` so it fills `target` without
/// stretching (CSS `object-fit: cover`), centred and mirrored like a selfie view.
fn cover_uv(tex: egui::Vec2, target: egui::Vec2) -> egui::Rect {
    let scale = (target.x / tex.x).max(target.y / tex.y);
    // Fraction of the image that stays visible along each axis (1.0 = all of it).
    let (u, v) = (target.x / (tex.x * scale), target.y / (tex.y * scale));
    // u runs right-to-left to mirror the image.
    egui::Rect::from_min_max(egui::pos2(0.5 + u / 2.0, 0.5 - v / 2.0), egui::pos2(0.5 - u / 2.0, 0.5 + v / 2.0))
}

impl eframe::App for DotMatrixApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_texture(&ctx);

        let status = self.source.status();
        // egui rebuilds the layout every frame, so "responsive" is just an `if`.
        if ui.available_width() < NARROW_WIDTH {
            self.mobile_layout(ui, &ctx, &status);
        } else {
            self.desktop_layout(ui, &status);
        }

        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::cover_uv;
    use eframe::egui::vec2;

    #[test]
    fn landscape_video_on_portrait_phone_crops_the_sides() {
        // 1280×720 camera on a 500×900 screen: full height visible, ~31% of the width.
        let uv = cover_uv(vec2(1280.0, 720.0), vec2(500.0, 900.0));
        assert!((uv.min.y - 0.0).abs() < 1e-6 && (uv.max.y - 1.0).abs() < 1e-6);
        assert!((uv.min.x - uv.max.x - 0.3125).abs() < 1e-6); // mirrored: min.x > max.x
        assert!(((uv.min.x + uv.max.x) / 2.0 - 0.5).abs() < 1e-6); // centred
    }

    #[test]
    fn same_aspect_shows_everything() {
        let uv = cover_uv(vec2(1280.0, 720.0), vec2(640.0, 360.0));
        assert_eq!((uv.min.x, uv.max.x, uv.min.y, uv.max.y), (1.0, 0.0, 0.0, 1.0));
    }
}
