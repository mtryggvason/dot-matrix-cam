//! Screenshots: encode the current frame as PNG, then save it.
//! Mac: a file in ~/Pictures. Browser: a file download.

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

/// Trigger a browser download. Returns a message for the user.
#[cfg(target_arch = "wasm32")]
pub fn save(png: &[u8]) -> Result<String, String> {
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    let err = |e: JsValue| format!("{e:?}");

    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let name = file_name((js_sys::Date::now() / 1000.0) as u64);

    // PNG bytes → Blob → temporary object URL → <a download> click.
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(png));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a").map_err(err)?.unchecked_into();
    link.set_href(&url);
    link.set_download(&name);
    link.click();

    // Free the object URL once the browser has had time to start the download.
    let revoke = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 30_000);
    Ok(format!("Downloaded {name}"))
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
