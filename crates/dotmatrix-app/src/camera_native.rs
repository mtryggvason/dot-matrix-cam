//! macOS camera via nokhwa (AVFoundation). Frames arrive on a background thread
//! and the UI thread picks up the newest one.

use std::sync::{Arc, Mutex};
use std::thread;

use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

use crate::FrameSource;

struct Frame {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
    seq: u64,
}

pub struct NativeCamera {
    latest: Arc<Mutex<Option<Frame>>>,
    status: Arc<Mutex<String>>,
    last_seq: u64,
}

impl NativeCamera {
    pub fn start() -> Self {
        let latest = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new("Starting camera…".to_owned()));
        let (l, s) = (latest.clone(), status.clone());
        // macOS: ask for camera permission first, then open the device.
        nokhwa::nokhwa_initialize(move |granted| {
            if granted {
                capture_loop(l.clone(), s.clone());
            } else {
                *s.lock().unwrap() = "Camera access denied. Allow it in System Settings → Privacy & Security → Camera.".into();
            }
        });
        Self { latest, status, last_seq: 0 }
    }
}

fn capture_loop(latest: Arc<Mutex<Option<Frame>>>, status: Arc<Mutex<String>>) {
    thread::spawn(move || {
        let set = |s: &str| *status.lock().unwrap() = s.to_owned();
        let fmt = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let mut cam = match Camera::new(CameraIndex::Index(0), fmt) {
            Ok(c) => c,
            Err(e) => return set(&format!("Camera error: {e}")),
        };
        if let Err(e) = cam.open_stream() {
            return set(&format!("Could not open camera: {e}"));
        }
        set("Running");
        let mut seq = 0;
        loop {
            match cam.frame().and_then(|f| f.decode_image::<RgbFormat>()) {
                Ok(img) => {
                    seq += 1;
                    let (width, height) = img.dimensions();
                    *latest.lock().unwrap() = Some(Frame { width, height, rgb: img.into_raw(), seq });
                }
                Err(e) => set(&format!("Frame error: {e}")),
            }
        }
    });
}

impl FrameSource for NativeCamera {
    fn poll(&mut self, force: bool, f: &mut dyn FnMut(u32, u32, usize, &[u8])) -> bool {
        let guard = self.latest.lock().unwrap();
        match guard.as_ref() {
            Some(fr) if force || fr.seq != self.last_seq => {
                self.last_seq = fr.seq;
                f(fr.width, fr.height, 3, &fr.rgb);
                true
            }
            _ => false,
        }
    }

    fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }
}
