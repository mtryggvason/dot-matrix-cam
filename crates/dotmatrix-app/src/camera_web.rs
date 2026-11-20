//! Browser camera via web-sys: getUserMedia → hidden <video> → canvas → RGBA pixels.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, MediaStream, MediaStreamConstraints};

use crate::FrameSource;

pub struct WebCamera {
    video: HtmlVideoElement,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    status: Rc<RefCell<String>>,
    last_time: f64,
}

impl WebCamera {
    pub fn start() -> Result<Self, JsValue> {
        let doc = web_sys::window().unwrap().document().unwrap();
        let video: HtmlVideoElement = doc.create_element("video")?.dyn_into()?;
        video.set_muted(true);
        video.set_attribute("playsinline", "")?;
        let canvas: HtmlCanvasElement = doc.create_element("canvas")?.dyn_into()?;
        let opts = js_sys::Object::new();
        js_sys::Reflect::set(&opts, &"willReadFrequently".into(), &true.into())?;
        let ctx: CanvasRenderingContext2d =
            canvas.get_context_with_context_options("2d", &opts)?.unwrap().dyn_into()?;

        let status = Rc::new(RefCell::new("Starting camera…".to_owned()));
        let (v, s) = (video.clone(), status.clone());
        spawn_local(async move {
            let result: Result<(), JsValue> = async {
                let devices = web_sys::window().unwrap().navigator().media_devices()?;
                let constraints = MediaStreamConstraints::new();
                constraints.set_video(&js_sys::JSON::parse(r#"{"width":1280,"height":720}"#)?);
                let stream: MediaStream =
                    JsFuture::from(devices.get_user_media_with_constraints(&constraints)?).await?.dyn_into()?;
                v.set_src_object(Some(&stream));
                JsFuture::from(v.play()?).await?;
                Ok(())
            }
            .await;
            *s.borrow_mut() = match result {
                Ok(()) => "Running".into(),
                Err(e) => format!("Camera error: {}", js_error_message(&e)),
            };
        });

        Ok(Self { video, canvas, ctx, status, last_time: -1.0 })
    }
}

fn js_error_message(e: &JsValue) -> String {
    js_sys::Reflect::get(e, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .unwrap_or_else(|| format!("{e:?}"))
}

impl FrameSource for WebCamera {
    fn poll(&mut self, force: bool, f: &mut dyn FnMut(u32, u32, usize, &[u8])) -> bool {
        let (w, h) = (self.video.video_width(), self.video.video_height());
        let t = self.video.current_time();
        if w == 0 || h == 0 || (!force && t == self.last_time) {
            return false;
        }
        self.last_time = t;
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        let ok = self.ctx.draw_image_with_html_video_element(&self.video, 0.0, 0.0).is_ok();
        match self.ctx.get_image_data(0.0, 0.0, w as f64, h as f64) {
            Ok(img) if ok => {
                f(w, h, 4, &img.data());
                true
            }
            _ => false,
        }
    }

    fn status(&self) -> String {
        self.status.borrow().clone()
    }
}
