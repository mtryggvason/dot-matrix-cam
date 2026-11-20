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

use dotmatrix_core::{Renderer, Settings};
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
    settings: Settings,
    /// Settings used for the current texture, to re-render when a control changes.
    rendered_with: Option<Settings>,
    renderer: Renderer,
    out: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    /// Called whenever the user changes a setting (e.g. to save it).
    on_change: Box<dyn FnMut(&Settings)>,
}

impl DotMatrixApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        source: Box<dyn FrameSource>,
        settings: Settings,
        on_change: Box<dyn FnMut(&Settings)>,
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
        }
    }

    /// Pull the newest camera frame and run the effect on it.
    fn update_texture(&mut self, ctx: &egui::Context) {
        let force = self.rendered_with != Some(self.settings);
        let Self { source, settings, renderer, out, texture, .. } = self;
        let rendered = source.poll(force, &mut |w, h, channels, px| {
            out.resize((w * h * 4) as usize, 0);
            if settings.enabled {
                renderer.render(px, channels, w, h, &settings.params, out);
            } else {
                for (o, i) in out.chunks_exact_mut(4).zip(px.chunks_exact(channels)) {
                    o.copy_from_slice(&[i[0], i[1], i[2], 255]);
                }
            }
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

impl eframe::App for DotMatrixApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_texture(&ctx);

        let status = self.source.status();
        egui::Panel::left("controls")
            .exact_size(280.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(ui::PANEL).inner_margin(20))
            .show(ui, |ui| {
                if ui::controls(ui, &mut self.settings, &status) {
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

        ctx.request_repaint();
    }
}
