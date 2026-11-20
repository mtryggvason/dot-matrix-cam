//! Theme and widgets for the app window (native and web).
//! Colours and spacing match the original HTML page.

use dotmatrix_core::{Mode, Params};
use eframe::egui::{self, Color32};

pub const BG: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x11);
pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x19, 0x1c);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x8f, 0x98);
pub const ACCENT: Color32 = Color32::from_rgb(0x39, 0xff, 0x14);
/// The mobile shelf: PANEL, slightly see-through so the video shows behind it.
pub const SHELF: Color32 = Color32::from_rgba_premultiplied(0x15, 0x17, 0x1a, 0xeb);
const INPUT: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
const BORDER: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const TRACK: Color32 = Color32::from_rgb(0x3a, 0x3d, 0x42);

pub fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.spacing.button_padding = egui::vec2(8.0, 6.0);
        s.spacing.interact_size.y = 28.0;
        let v = &mut s.visuals;
        v.override_text_color = Some(TEXT);
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = INPUT;
        v.window_stroke = egui::Stroke::new(1.0, BORDER);
        v.window_corner_radius = 8.into();
        v.menu_corner_radius = 6.into();
        v.selection.bg_fill = ACCENT.linear_multiply(0.25);
        v.selection.stroke = egui::Stroke::new(1.0, TEXT);
        for (w, fill) in [
            (&mut v.widgets.inactive, INPUT),
            (&mut v.widgets.hovered, Color32::from_rgb(0x2a, 0x2a, 0x2a)),
            (&mut v.widgets.active, Color32::from_rgb(0x30, 0x30, 0x30)),
            (&mut v.widgets.open, INPUT),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.bg_stroke = egui::Stroke::new(1.0, BORDER);
            w.fg_stroke = egui::Stroke::new(1.0, TEXT);
            w.corner_radius = 6.into();
            w.expansion = 0.0;
        }
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    });
}

/// The whole control column. Returns true if anything changed.
pub fn controls(ui: &mut egui::Ui, p: &mut Params, status: &str) -> bool {
    let before = *p;
    ui.spacing_mut().item_spacing.y = 16.0;

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(egui::RichText::new("Dot Matrix Cam").size(18.0).strong().color(TEXT));
        ui.label(egui::RichText::new(status).size(12.0).color(MUTED));
    });

    let mut cell = p.cell as f32;
    labeled_slider(ui, "Grid (smaller = more dots)", &mut cell, 3.0..=48.0, 1.0, 0);
    p.cell = cell as u32;
    labeled_slider(ui, "Dot size", &mut p.dot_scale, 0.1..=1.5, 0.05, 2);
    labeled_slider(ui, "Contrast", &mut p.contrast, 0.5..=3.0, 0.05, 2);

    row(ui, "Dot colour", |ui| color_swatch(ui, &mut p.fg));
    row(ui, "Background", |ui| color_swatch(ui, &mut p.bg));
    row(ui, "Mode", |ui| {
        let text = if p.mode == Mode::Size { "Halftone" } else { "LED" };
        egui::ComboBox::from_id_salt("mode").selected_text(text).width(110.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut p.mode, Mode::Size, "Halftone");
            ui.selectable_value(&mut p.mode, Mode::Brightness, "LED");
        });
    });
    row(ui, "Invert", |ui| checkbox(ui, &mut p.invert));

    *p != before
}

/// Full-width tap target at the top of the mobile shelf: a grab bar, a label and a
/// chevron pointing the way the shelf will move. Returns the click response.
pub fn shelf_handle(ui: &mut egui::Ui, open: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
    let p = ui.painter();

    // Grab bar, centred at the top.
    let bar = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 6.0), egui::vec2(36.0, 4.0));
    p.rect_filled(bar, 2.0, TRACK);

    // "Controls" on the left, chevron on the right.
    let y = rect.center().y + 4.0;
    let color = if response.hovered() { TEXT } else { MUTED };
    p.text(egui::pos2(rect.left(), y), egui::Align2::LEFT_CENTER, "Controls", egui::FontId::proportional(14.0), color);
    let (cx, d) = (rect.right() - 8.0, if open { 3.0 } else { -3.0 }); // open: points down (close)
    let stroke = egui::Stroke::new(2.0, color);
    p.line_segment([egui::pos2(cx - 6.0, y - d), egui::pos2(cx, y + d)], stroke);
    p.line_segment([egui::pos2(cx, y + d), egui::pos2(cx + 6.0, y - d)], stroke);

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// "Label            value" on one line, a full-width slider underneath.
fn labeled_slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    decimals: usize,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.*}", decimals, *value));
            });
        });
        slider(ui, value, range, step);
    });
}

/// Thin rounded track with accent fill and a round accent thumb (like CSS `accent-color`).
fn slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, step: f32) {
    let (min, max) = (*range.start(), *range.end());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::click_and_drag());
    let thumb_r = 8.0;
    let (x0, x1) = (rect.left() + thumb_r, rect.right() - thumb_r);

    if let Some(pos) = response.interact_pointer_pos() {
        let t = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        *value = (((min + t * (max - min)) / step).round() * step).clamp(min, max);
    }

    let x = x0 + (*value - min) / (max - min) * (x1 - x0);
    let y = rect.center().y;
    let p = ui.painter();
    let track = egui::Rect::from_min_max(egui::pos2(x0, y - 2.5), egui::pos2(x1, y + 2.5));
    p.rect_filled(track, 3.0, TRACK);
    p.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(x, track.max.y)), 3.0, ACCENT);
    let active = response.hovered() || response.dragged();
    p.circle_filled(egui::pos2(x, y), if active { thumb_r + 1.0 } else { thumb_r }, ACCENT);
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// Muted label on the left, control on the right.
fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    });
}

/// 44×28 colour swatch that opens egui's colour picker.
fn color_swatch(ui: &mut egui::Ui, rgb: &mut [u8; 3]) {
    ui.spacing_mut().interact_size = egui::vec2(44.0, 28.0);
    let mut c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    ui.color_edit_button_srgba(&mut c);
    *rgb = [c.r(), c.g(), c.b()];
}

/// Rounded checkbox that fills with the accent colour when checked.
fn checkbox(ui: &mut egui::Ui, on: &mut bool) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let p = ui.painter();
    if *on {
        p.rect_filled(rect, 3.0, ACCENT);
        let s = egui::Stroke::new(2.0, BG);
        let a = rect.left_center() + egui::vec2(3.5, 0.5);
        let b = rect.center_bottom() + egui::vec2(-1.0, -4.0);
        let c = rect.right_top() + egui::vec2(-3.5, 4.0);
        p.line_segment([a, b], s);
        p.line_segment([b, c], s);
    } else {
        p.rect(rect, 3.0, INPUT, egui::Stroke::new(1.0, Color32::from_gray(0x77)), egui::StrokeKind::Inside);
    }
}
