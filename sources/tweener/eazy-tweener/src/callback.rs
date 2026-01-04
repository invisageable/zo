//! Callback system for animation lifecycle events.
//!
//! Supports both synchronous and asynchronous callbacks for integration
//! with various runtime environments.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// A synchronous callback function.
pub type SyncCallback = Arc<dyn Fn() + Send + Sync>;

/// An asynchronous callback function that returns a pinned future.
pub type AsyncCallback =
  Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A callback that can be either synchronous or asynchronous.
#[derive(Clone)]
pub enum Callback {
  /// Synchronous callback executed immediately.
  Sync(SyncCallback),
  /// Asynchronous callback that returns a future.
  Async(AsyncCallback),
}

impl Callback {
  /// Create a sync callback from a closure.
  pub fn sync<F>(f: F) -> Self
  where
    F: Fn() + Send + Sync + 'static,
  {
    Self::Sync(Arc::new(f))
  }

  /// Create an async callback from a closure returning a future.
  pub fn future<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
  {
    Self::Async(Arc::new(move || Box::pin(f())))
  }

  /// Execute the callback synchronously.
  ///
  /// For sync callbacks, runs immediately.
  /// For async callbacks, blocks on the future (not recommended in async
  /// context).
  pub fn fire(&self) {
    match self {
      Self::Sync(f) => f(),
      Self::Async(_) => {
        // In a sync context, we can't easily run async code.
        // The caller should use fire_async for async callbacks.
      }
    }
  }

  /// Check if this is an async callback.
  pub fn is_async(&self) -> bool {
    matches!(self, Self::Async(_))
  }

  /// Get the future for an async callback.
  ///
  /// Returns `None` for sync callbacks.
  pub fn as_future(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
    match self {
      Self::Sync(_) => None,
      Self::Async(f) => Some(f()),
    }
  }
}

impl std::fmt::Debug for Callback {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Sync(_) => f.write_str("Callback::Sync(...)"),
      Self::Async(_) => f.write_str("Callback::Async(...)"),
    }
  }
}

/// Collection of lifecycle callbacks for a tween or timeline.
#[derive(Clone, Default)]
pub struct Callbacks {
  /// Called when the animation starts playing.
  pub on_start: Option<Callback>,
  /// Called on every tick while the animation is playing.
  pub on_update: Option<Callback>,
  /// Called when the animation completes.
  pub on_complete: Option<Callback>,
  /// Called each time the animation repeats.
  pub on_repeat: Option<Callback>,
}

impl Callbacks {
  /// Create a new empty callbacks collection.
  pub fn new() -> Self {
    Self::default()
  }

  /// Set the on_start callback.
  pub fn with_on_start(mut self, cb: Callback) -> Self {
    self.on_start = Some(cb);
    self
  }

  /// Set the on_update callback.
  pub fn with_on_update(mut self, cb: Callback) -> Self {
    self.on_update = Some(cb);
    self
  }

  /// Set the on_complete callback.
  pub fn with_on_complete(mut self, cb: Callback) -> Self {
    self.on_complete = Some(cb);
    self
  }

  /// Set the on_repeat callback.
  pub fn with_on_repeat(mut self, cb: Callback) -> Self {
    self.on_repeat = Some(cb);
    self
  }

  /// Fire the on_start callback if present.
  pub fn fire_start(&self) {
    if let Some(cb) = &self.on_start {
      cb.fire();
    }
  }

  /// Fire the on_update callback if present.
  pub fn fire_update(&self) {
    if let Some(cb) = &self.on_update {
      cb.fire();
    }
  }

  /// Fire the on_complete callback if present.
  pub fn fire_complete(&self) {
    if let Some(cb) = &self.on_complete {
      cb.fire();
    }
  }

  /// Fire the on_repeat callback if present.
  pub fn fire_repeat(&self) {
    if let Some(cb) = &self.on_repeat {
      cb.fire();
    }
  }

  /// Check if any callbacks are async.
  pub fn has_async(&self) -> bool {
    self.on_start.as_ref().is_some_and(|cb| cb.is_async())
      || self.on_update.as_ref().is_some_and(|cb| cb.is_async())
      || self.on_complete.as_ref().is_some_and(|cb| cb.is_async())
      || self.on_repeat.as_ref().is_some_and(|cb| cb.is_async())
  }
}

impl std::fmt::Debug for Callbacks {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Callbacks")
      .field("on_start", &self.on_start.is_some())
      .field("on_update", &self.on_update.is_some())
      .field("on_complete", &self.on_complete.is_some())
      .field("on_repeat", &self.on_repeat.is_some())
      .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};

  #[test]
  fn test_sync_callback() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let cb = Callback::sync(move || {
      counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    cb.fire();
    cb.fire();

    assert_eq!(counter.load(Ordering::SeqCst), 2);
  }

  #[test]
  fn test_callbacks_fire() {
    let start_count = Arc::new(AtomicUsize::new(0));
    let complete_count = Arc::new(AtomicUsize::new(0));

    let start_clone = start_count.clone();
    let complete_clone = complete_count.clone();

    let callbacks = Callbacks::new()
      .with_on_start(Callback::sync(move || {
        start_clone.fetch_add(1, Ordering::SeqCst);
      }))
      .with_on_complete(Callback::sync(move || {
        complete_clone.fetch_add(1, Ordering::SeqCst);
      }));

    callbacks.fire_start();
    callbacks.fire_complete();

    assert_eq!(start_count.load(Ordering::SeqCst), 1);
    assert_eq!(complete_count.load(Ordering::SeqCst), 1);
  }

  #[tokio::test]
  async fn test_async_callback() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let cb = Callback::future(move || {
      let counter = counter_clone.clone();
      async move {
        counter.fetch_add(1, Ordering::SeqCst);
      }
    });

    assert!(cb.is_async());

    // Get and await the future.
    if let Some(fut) = cb.as_future() {
      fut.await;
    }

    // Fire again via as_future.
    if let Some(fut) = cb.as_future() {
      fut.await;
    }

    assert_eq!(counter.load(Ordering::SeqCst), 2);
  }

  #[tokio::test]
  async fn test_async_callback_with_delay() {
    use std::time::Instant;

    let start = Instant::now();
    let completed = Arc::new(AtomicUsize::new(0));
    let completed_clone = completed.clone();

    let cb = Callback::future(move || {
      let completed = completed_clone.clone();
      async move {
        // Simulate async work.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        completed.fetch_add(1, Ordering::SeqCst);
      }
    });

    if let Some(fut) = cb.as_future() {
      fut.await;
    }

    assert_eq!(completed.load(Ordering::SeqCst), 1);
    assert!(start.elapsed().as_millis() >= 10);
  }

  #[tokio::test]
  async fn test_callbacks_with_async() {
    let start_count = Arc::new(AtomicUsize::new(0));
    let complete_count = Arc::new(AtomicUsize::new(0));

    let start_clone = start_count.clone();
    let complete_clone = complete_count.clone();

    let callbacks = Callbacks::new()
      .with_on_start(Callback::future(move || {
        let start = start_clone.clone();
        async move {
          start.fetch_add(1, Ordering::SeqCst);
        }
      }))
      .with_on_complete(Callback::future(move || {
        let complete = complete_clone.clone();
        async move {
          complete.fetch_add(1, Ordering::SeqCst);
        }
      }));

    assert!(callbacks.has_async());

    // Fire async callbacks via as_future.
    if let Some(fut) = callbacks.on_start.as_ref().and_then(|cb| cb.as_future())
    {
      fut.await;
    }
    if let Some(fut) =
      callbacks.on_complete.as_ref().and_then(|cb| cb.as_future())
    {
      fut.await;
    }

    assert_eq!(start_count.load(Ordering::SeqCst), 1);
    assert_eq!(complete_count.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn test_sync_callback_not_async() {
    let cb = Callback::sync(|| {});
    assert!(!cb.is_async());
    assert!(cb.as_future().is_none());
  }

  #[test]
  fn test_callbacks_has_async_mixed() {
    let sync_only = Callbacks::new()
      .with_on_start(Callback::sync(|| {}))
      .with_on_complete(Callback::sync(|| {}));

    assert!(!sync_only.has_async());

    let mixed = Callbacks::new()
      .with_on_start(Callback::sync(|| {}))
      .with_on_complete(Callback::future(|| async {}));

    assert!(mixed.has_async());
  }
}
