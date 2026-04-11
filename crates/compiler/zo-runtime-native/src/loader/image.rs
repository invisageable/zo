//! Async image loader for the native (egui) runtime.
//!
//! Images are decoded on a worker thread to avoid blocking
//! the UI thread. The main thread uploads decoded pixels to
//! the GPU (egui texture constraint) on the frame after the
//! decode completes.
//!
//! ### State Machine
//!
//! ```text
//! Pending --[send LoadRequest]----> Loading
//! Loading --[recv LoadResponse::Ok]-> Decoded
//! Loading --[recv LoadResponse::Err]-> Failed
//! Decoded --[main thread uploads]--> Ready
//! ```

use eframe::egui;

use rustc_hash::FxHashMap as HashMap;

use std::sync::mpsc;
use std::thread;

/// Lifecycle of a single image.
pub enum ImageState {
  /// Cache miss — not yet requested.
  Pending,
  /// Worker is reading/decoding.
  Loading,
  /// Decoded pixels, waiting for GPU upload.
  Decoded(egui::ColorImage),
  /// Texture ready to render.
  Ready(egui::TextureHandle),
  /// I/O or decode error.
  Failed(String),
}

/// Main thread → worker thread.
struct LoadRequest {
  src: String,
}

/// Worker thread → main thread.
enum LoadResponse {
  Ok {
    src: String,
    image: egui::ColorImage,
  },
  Err {
    src: String,
    error: String,
  },
}

/// Async image loader. Holds a background worker thread,
/// a cache of image states, and channels to communicate
/// with the worker.
pub struct ImageLoader {
  cache: HashMap<String, ImageState>,
  request_tx: mpsc::Sender<LoadRequest>,
  response_rx: mpsc::Receiver<LoadResponse>,
  _worker: thread::JoinHandle<()>,
}

impl ImageLoader {
  /// Creates a new loader and spawns the worker thread.
  pub fn new() -> Self {
    let (request_tx, request_rx) = mpsc::channel::<LoadRequest>();
    let (response_tx, response_rx) = mpsc::channel::<LoadResponse>();

    let worker = thread::spawn(move || {
      worker_loop(request_rx, response_tx);
    });

    Self {
      cache: HashMap::default(),
      request_tx,
      response_rx,
      _worker: worker,
    }
  }

  /// Drain the worker response channel and transition
  /// matching cache entries to `Decoded` or `Failed`.
  /// Call this once per frame before `state()`.
  pub fn poll(&mut self) {
    while let Ok(response) = self.response_rx.try_recv() {
      match response {
        LoadResponse::Ok { src, image } => {
          self.cache.insert(src, ImageState::Decoded(image));
        }
        LoadResponse::Err { src, error } => {
          self.cache.insert(src, ImageState::Failed(error));
        }
      }
    }
  }

  /// Return the current state for `src`. On first call
  /// (cache miss), sends a `LoadRequest` to the worker
  /// and transitions to `Loading`.
  pub fn state(&mut self, src: &str) -> &mut ImageState {
    if !self.cache.contains_key(src) {
      self.cache.insert(src.to_string(), ImageState::Pending);
    }

    let state = self.cache.get_mut(src).unwrap();

    if matches!(state, ImageState::Pending) {
      let req = LoadRequest {
        src: src.to_string(),
      };

      if self.request_tx.send(req).is_ok() {
        *state = ImageState::Loading;
      } else {
        *state = ImageState::Failed("worker thread died".to_string());
      }
    }

    state
  }
}

impl Default for ImageLoader {
  fn default() -> Self {
    Self::new()
  }
}

/// Worker thread body. Reads image files and decodes them,
/// sending results back via `tx`. Exits when `rx` closes.
fn worker_loop(
  rx: mpsc::Receiver<LoadRequest>,
  tx: mpsc::Sender<LoadResponse>,
) {
  while let Ok(req) = rx.recv() {
    let response = decode(&req.src);

    if tx.send(response).is_err() {
      break;
    }
  }
}

/// Read and decode a single image file.
fn decode(src: &str) -> LoadResponse {
  let bytes = match std::fs::read(src) {
    Ok(b) => b,
    Err(e) => {
      return LoadResponse::Err {
        src: src.to_string(),
        error: format!("read error: {e}"),
      };
    }
  };

  let img = match image::load_from_memory(&bytes) {
    Ok(i) => i,
    Err(e) => {
      return LoadResponse::Err {
        src: src.to_string(),
        error: format!("decode error: {e}"),
      };
    }
  };

  let rgba = img.to_rgba8();
  let size = [rgba.width() as usize, rgba.height() as usize];
  let pixels = rgba.as_flat_samples();
  let color_image =
    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

  LoadResponse::Ok {
    src: src.to_string(),
    image: color_image,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_initial_state_is_loading_after_first_query() {
    let mut loader = ImageLoader::new();

    // First query → Pending → immediately transitions to
    // Loading (request sent to worker).
    let state = loader.state("/nonexistent.png");

    assert!(matches!(state, ImageState::Loading));
  }

  #[test]
  fn test_nonexistent_file_becomes_failed() {
    let mut loader = ImageLoader::new();

    loader.state("/definitely/does/not/exist.png");

    // Wait for worker to process the request.
    let mut attempts = 0;

    loop {
      loader.poll();

      let state = loader.state("/definitely/does/not/exist.png");

      if matches!(state, ImageState::Failed(_)) {
        break;
      }

      attempts += 1;

      if attempts > 100 {
        panic!("worker never produced a Failed state");
      }

      std::thread::sleep(std::time::Duration::from_millis(10));
    }
  }

  #[test]
  fn test_same_src_queried_twice_only_loads_once() {
    let mut loader = ImageLoader::new();

    // First query: Pending → Loading (sends request).
    loader.state("/x.png");

    // Second query: still Loading (no new request).
    let state = loader.state("/x.png");

    assert!(matches!(state, ImageState::Loading));
  }
}
