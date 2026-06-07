//! Shared image-source byte loader for every native runtime.
//!
//! One path for "given a source, get the bytes": an `http(s)://` URL
//! or a local file. Each runtime decodes those bytes its own way (egui
//! into a texture, UIKit into a `UIImage`), but the fetch is identical,
//! so it lives here instead of being duplicated per backend.

/// Bytes for an image source: an `http(s)://` URL via a blocking GET,
/// else a local file read. The caller decodes them.
pub fn load_image_bytes(src: &str) -> Result<Vec<u8>, String> {
  if is_http_url(src) {
    fetch_http(src)
  } else {
    std::fs::read(src).map_err(|error| format!("read error: {error}"))
  }
}

/// Whether `src` is an `http://` or `https://` URL.
fn is_http_url(src: &str) -> bool {
  src.starts_with("http://") || src.starts_with("https://")
}

/// Blocking HTTP GET — fetches the full body into a `Vec`.
#[cfg(feature = "http")]
fn fetch_http(url: &str) -> Result<Vec<u8>, String> {
  let mut response = ureq::get(url)
    .call()
    .map_err(|error| format!("http error: {error}"))?;

  response
    .body_mut()
    .read_to_vec()
    .map_err(|error| format!("http read error: {error}"))
}

/// Without the `http` feature (the iOS runtime), URL sources are
/// unreachable — a device loads bundled local assets instead.
#[cfg(not(feature = "http"))]
fn fetch_http(_url: &str) -> Result<Vec<u8>, String> {
  Err("http image sources need the `http` feature".to_string())
}
