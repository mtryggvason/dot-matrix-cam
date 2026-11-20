//! Native macOS entry point: `cargo run --release -p dotmatrix-app`.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use dotmatrix_app::{native_camera, DotMatrixApp};
    use eframe::egui;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 620.0])
            .with_title("Dot Matrix Cam"),
        ..Default::default()
    };
    eframe::run_native(
        "Dot Matrix Cam",
        options,
        Box::new(|cc| Ok(Box::new(DotMatrixApp::new(cc, native_camera(), Default::default(), Box::new(|_| {}))))),
    )
}

// The browser build uses `start_app` in src/web.rs instead.
#[cfg(target_arch = "wasm32")]
fn main() {}
