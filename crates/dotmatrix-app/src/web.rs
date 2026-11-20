//! WebAssembly entry point, called from the loader script in web/index.html.
//! Runs the full app on a web page; settings are saved in localStorage.

use dotmatrix_core::Settings;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use crate::{camera_web::WebCamera, DotMatrixApp};

const KEY: &str = "settings";

fn canvas(id: &str) -> Result<HtmlCanvasElement, JsValue> {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .ok_or_else(|| JsValue::from_str(&format!("no element #{id}")))?
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("#{id} is not a <canvas>")))
}

fn parse(json: Option<String>) -> Settings {
    json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

#[wasm_bindgen]
pub async fn start_app(canvas_id: &str) -> Result<(), JsValue> {
    let storage = web_sys::window().unwrap().local_storage().ok().flatten();
    let settings = parse(storage.as_ref().and_then(|s| s.get_item(KEY).ok().flatten()));
    let camera = WebCamera::start()?;
    eframe::WebRunner::new()
        .start(
            canvas(canvas_id)?,
            eframe::WebOptions::default(),
            Box::new(move |cc| {
                let save = move |s: &Settings| {
                    if let (Some(st), Ok(json)) = (&storage, serde_json::to_string(s)) {
                        let _ = st.set_item(KEY, &json);
                    }
                };
                Ok(Box::new(DotMatrixApp::new(cc, Box::new(camera), settings, Box::new(save))))
            }),
        )
        .await
}
