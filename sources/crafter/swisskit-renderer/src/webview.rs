//! WebView wrapper for cross-platform HTML rendering.
//!
//! Provides a generic WebView component using wry that can be embedded
//! in egui applications for rendering HTML content.

use raw_window_handle::{
  HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebViewBuilder};

/// WebView wrapper for embedding HTML content in applications.
///
/// Uses wry for cross-platform WebView support (WKWebView on macOS,
/// WebView2 on Windows, WebKitGTK on Linux).
pub struct WebView {
  /// The URL currently loaded in the WebView.
  pub url: String,
  /// The underlying wry WebView instance.
  pub webview: Option<wry::WebView>,
  /// Raw window handle for embedding the WebView.
  pub window_handle_raw: Option<RawWindowHandle>,
  /// Whether the WebView is currently visible.
  pub visible: bool,
}

impl WebView {
  /// Creates a new WebView with the specified URL.
  pub fn new(url: impl Into<String>) -> Self {
    Self {
      url: url.into(),
      webview: None,
      window_handle_raw: None,
      visible: false,
    }
  }

  /// Shows the WebView.
  pub fn show(&mut self) {
    self.visible = true;
  }

  /// Hides the WebView by moving it off-screen.
  pub fn hide(&mut self) {
    self.visible = false;
    if let Some(webview) = &self.webview {
      let off_screen_bounds = Rect {
        position: LogicalPosition::new(-10000.0, -10000.0).into(),
        size: LogicalSize::new(1.0, 1.0).into(),
      };

      let _ = webview.set_bounds(off_screen_bounds);
    }
  }

  /// Sets the window handle for WebView creation.
  pub fn set_window_handle(
    &mut self,
    handle: raw_window_handle::RawWindowHandle,
  ) {
    self.window_handle_raw = Some(handle);
  }

  /// Loads a URL in the WebView.
  pub fn load_url(&mut self, url: impl Into<String>) {
    self.url = url.into();
    log::debug!("[WebView] load_url called with: {}", self.url);

    if self.webview.is_none() {
      log::debug!("[WebView] WebView doesn't exist, trying to create");

      self.try_create_webview();
    } else if let Some(webview) = &self.webview {
      log::debug!("[WebView] WebView exists, loading URL");

      let _ = webview.load_url(&self.url);
    }
  }

  /// Reloads the current URL in the WebView.
  pub fn reload(&self) {
    if let Some(webview) = &self.webview {
      log::debug!("[WebView] Reloading WebView");

      let _ = webview.load_url(&self.url);
    }
  }

  /// Attempts to create the WebView if window handle and URL are available.
  pub fn try_create_webview(&mut self) {
    if let Some(handle) = &self.window_handle_raw {
      if !self.url.is_empty() {
        log::debug!(
          "[WebView] Attempting to create WebView with URL: {}",
          self.url
        );

        match self.create_webview(*handle) {
          Ok(()) => {
            log::info!(
              "[WebView] WebView created successfully for URL: {}",
              self.url
            );
          }
          Err(e) => {
            log::error!("[WebView] Failed to create WebView: {e}");
          }
        }
      } else {
        log::warn!("[WebView] Cannot create WebView - URL is empty");
      }
    } else {
      log::warn!("[WebView] Cannot create WebView - no window handle");
    }
  }

  /// Updates the WebView bounds to match the given rectangle.
  pub fn update_bounds(&self, x: f64, y: f64, width: f64, height: f64) {
    if let Some(webview) = &self.webview
      && self.visible
    {
      let bounds = Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(width, height).into(),
      };

      let _ = webview.set_bounds(bounds);
    }
  }

  /// Gets the current URL.
  pub fn current_url(&self) -> &str {
    &self.url
  }

  /// Returns whether the WebView has been created.
  pub fn is_created(&self) -> bool {
    self.webview.is_some()
  }

  /// Creates and positions the WebView as a child of the main window.
  fn create_webview(
    &mut self,
    handle: RawWindowHandle,
  ) -> Result<(), wry::Error> {
    let bounds = Rect {
      position: LogicalPosition::new(0.0, 0.0).into(),
      size: LogicalSize::new(400.0, 600.0).into(),
    };

    // Temporary struct that implements HasWindowHandle.
    struct TempWindowHandle(RawWindowHandle);

    impl HasWindowHandle for TempWindowHandle {
      fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: The handle is valid for the lifetime of this borrow.
        unsafe { Ok(WindowHandle::borrow_raw(self.0)) }
      }
    }

    let temp_handle = TempWindowHandle(handle);

    let webview = WebViewBuilder::new()
      .with_url(&self.url)
      .with_bounds(bounds)
      .build_as_child(&temp_handle)?;

    self.webview = Some(webview);

    Ok(())
  }
}

impl Default for WebView {
  fn default() -> Self {
    Self::new("")
  }
}

impl Drop for WebView {
  fn drop(&mut self) {
    self.webview = None;
    self.window_handle_raw = None;
  }
}
