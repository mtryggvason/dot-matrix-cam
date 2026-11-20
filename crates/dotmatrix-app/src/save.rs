//! Screenshots: encode the current frame as PNG, then save it.
//! Mac: a file in ~/Pictures. Phones: the native share sheet. Other browsers: a download.

/// RGBA frame → PNG bytes, mirrored left-right so it matches the on-screen (selfie) view.
pub fn encode_png_mirrored(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let row_bytes = width as usize * 4;
    let mut mirrored = Vec::with_capacity(rgba.len());
    for row in rgba.chunks_exact(row_bytes).take(height as usize) {
        for pixel in row.chunks_exact(4).rev() {
            mirrored.extend_from_slice(pixel);
        }
    }

    let mut png_bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&mirrored).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(png_bytes)
}

/// `dot-matrix-2026-11-20-131745.png` (UTC) from seconds since 1970.
fn file_name(unix_secs: u64) -> String {
    // Days since 1970 → calendar date (Howard Hinnant's civil_from_days algorithm).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let s = unix_secs % 86_400;
    format!("dot-matrix-{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}.png", s / 3600, s / 60 % 60, s % 60)
}

/// Save to ~/Pictures. Returns a message for the user.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(png: &[u8]) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let dir = home
        .as_ref()
        .map(|h| h.join("Pictures"))
        .filter(|d| d.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(file_name(secs));
    std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
    // Show ~/… instead of /Users/name/…
    let shown = match &home {
        Some(h) => path.strip_prefix(h).map(|p| format!("~/{}", p.display())).unwrap_or_else(|_| path.display().to_string()),
        None => path.display().to_string(),
    };
    Ok(format!("Saved {shown}"))
}

/// Browser: on touch devices (phones, tablets) open the native share sheet, so the
/// image can go to Photos, AirDrop, Messages…; elsewhere download the file.
/// Returns a message for the user (empty when the share sheet speaks for itself).
#[cfg(target_arch = "wasm32")]
pub fn save(png: &[u8]) -> Result<String, String> {
    let name = file_name((js_sys::Date::now() / 1000.0) as u64);
    if is_touch_device() && share(png, &name)? {
        return Ok(String::new());
    }
    download(png, &name)?;
    Ok(format!("Downloaded {name}"))
}

/// Phones and tablets: the primary pointer is a finger.
#[cfg(target_arch = "wasm32")]
fn is_touch_device() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(pointer: coarse)").ok().flatten())
        .is_some_and(|m| m.matches())
}

#[cfg(target_arch = "wasm32")]
fn js_err(e: wasm_bindgen::JsValue) -> String {
    format!("{e:?}")
}

/// Open the share sheet with the PNG as a file. Returns false if this browser
/// can't share files, so the caller can fall back to a download.
#[cfg(target_arch = "wasm32")]
fn share(png: &[u8], name: &str) -> Result<bool, String> {
    let navigator = web_sys::window().ok_or("no window")?.navigator();
    // Older browsers have no navigator.canShare at all; calling it would throw.
    if !js_sys::Reflect::has(&navigator, &"canShare".into()).unwrap_or(false) {
        return Ok(false);
    }
    let options = web_sys::FilePropertyBag::new();
    options.set_type("image/png");
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(png));
    let file = web_sys::File::new_with_u8_array_sequence_and_options(&parts, name, &options).map_err(js_err)?;
    let data = web_sys::ShareData::new();
    data.set_files(&js_sys::Array::of1(&file));
    if !navigator.can_share_with_data(&data) {
        return Ok(false);
    }

    // share() must start during the tap, so call it now and only await the result.
    let promise = navigator.share_with_data(&data);
    let (png, name) = (png.to_vec(), name.to_owned());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
            let reason = js_sys::Reflect::get(&e, &"name".into()).ok().and_then(|n| n.as_string());
            // AbortError = the user closed the share sheet. Anything else: download instead.
            if reason.as_deref() != Some("AbortError") {
                let _ = download(&png, &name);
            }
        }
    });
    Ok(true)
}

/// PNG bytes → Blob → temporary object URL → <a download> click.
#[cfg(target_arch = "wasm32")]
fn download(png: &[u8], name: &str) -> Result<(), String> {
    use wasm_bindgen::{closure::Closure, JsCast};

    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(png));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js_err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_err)?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js_err)?.unchecked_into();
    link.set_href(&url);
    link.set_download(name);
    link.click();

    // Free the object URL once the browser has had time to start the download.
    let revoke = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 30_000);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        assert_eq!(file_name(0), "dot-matrix-1970-01-01-000000.png");
        assert_eq!(file_name(1_700_000_000), "dot-matrix-2023-11-14-221320.png");
        assert_eq!(file_name(951_782_400), "dot-matrix-2000-02-29-000000.png"); // leap day
    }

    #[test]
    fn png_is_mirrored_and_decodes() {
        // 2×1 image: red, then blue. Mirrored: blue, then red.
        let png_bytes = encode_png_mirrored(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes)).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buf).unwrap();
        assert_eq!(&buf[..8], &[0, 0, 255, 255, 255, 0, 0, 255]);
    }
}
