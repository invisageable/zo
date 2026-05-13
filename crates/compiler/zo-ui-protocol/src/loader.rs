//! Dynamic library loader for compiled zo applications.
//!
//! **Currently unused; latent shape mismatch.** The
//! codegen's `_zo_ui_entry_point` returns `*const u8`
//! (pointer to the template's postcard payload), but
//! `load` here interprets the returned pointer as a
//! `*const UiTemplateBlob` (ptr + len header). Deref'ing
//! the first 8 bytes of postcard as a pointer segfaults.
//!
//! The discrepancy doesn't bite today because nothing in
//! the workspace sets `RuntimeConfig.library_path`, so
//! `load` is never called. Fixing requires either:
//!
//! - codegen emits a `UiTemplateBlob` literal in the
//!   data segment and `_zo_ui_entry_point` returns its
//!   address; OR
//! - codegen exposes a second symbol (e.g.
//!   `_zo_ui_entry_len`) so this loader can read the
//!   length without a wrapping struct.
//!
//! The `zo build` → `./exe` → `_zo_run_native` path used
//! by every compiled zsx program bypasses this loader
//! entirely (the exe links the runtime dylib directly and
//! passes its own bytes via `ZoRuntimeContext`).

use crate::codec;
use crate::ui_protocol::UiCommand;

use libloading::{Library, Symbol};
use thin_vec::ThinVec;

use std::ffi::c_void;
use std::slice;

/// `_zo_ui_entry_point` returns a pointer to this header,
/// followed immediately in memory by `len` bytes of
/// postcard-encoded `Vec<UiCommand>`.
#[repr(C)]
pub struct UiTemplateBlob {
  pub ptr: *const u8,
  pub len: usize,
}

/// Compiled binary's UI entry point. Returns the template
/// blob; runtime decodes it via `codec::decode`.
pub type UiEntryPoint = unsafe extern "C" fn() -> *const UiTemplateBlob;

/// Optional event dispatcher exported by the compiled exe.
/// Reactive binaries (post-P3 of `PLAN_DOM_CODEGEN_WIRING`)
/// expose this; static templates omit it.
pub type EventHandler = unsafe extern "C" fn(*mut c_void, u32, *mut c_void);

/// Loads and inspects a compiled zo dylib. Owns the loaded
/// `Library` for the lifetime of the loader so symbols
/// stay live.
pub struct LibraryLoader {
  library: Option<Library>,
  event_handler: Option<Symbol<'static, EventHandler>>,
}

impl LibraryLoader {
  /// Creates a new [`LibraryLoader`] instance.
  pub fn new() -> Self {
    Self {
      library: None,
      event_handler: None,
    }
  }

  /// Open `path`, call `_zo_ui_entry_point`, decode the
  /// returned blob into a command stream.
  pub fn load(
    &mut self,
    path: &str,
  ) -> Result<ThinVec<UiCommand>, Box<dyn std::error::Error>> {
    let lib = unsafe { Library::new(path) }?;

    let entry: Symbol<UiEntryPoint> =
      unsafe { lib.get(b"_zo_ui_entry_point") }?;

    if let Ok(handler) =
      unsafe { lib.get::<Symbol<EventHandler>>(b"_zo_handle_event") }
    {
      // Leak the symbol to 'static — its lifetime is tied
      // to `self.library`, which we keep until Drop.
      self.event_handler = Some(unsafe {
        std::mem::transmute::<
          Symbol<'_, Symbol<'_, EventHandler>>,
          Symbol<'static, EventHandler>,
        >(handler)
      });
    }

    let blob_ptr = unsafe { entry() };

    if blob_ptr.is_null() {
      self.library = Some(lib);
      return Ok(ThinVec::new());
    }

    let blob = unsafe { &*blob_ptr };
    let bytes = unsafe { slice::from_raw_parts(blob.ptr, blob.len) };
    let cmds = codec::decode(bytes)?;

    self.library = Some(lib);

    Ok(cmds.into_iter().collect())
  }

  /// Call the event handler if available.
  pub fn handle_event(
    &self,
    widget_id: *mut c_void,
    event_type: u32,
    event_data: *mut c_void,
  ) {
    if let Some(ref handler) = self.event_handler {
      unsafe { handler(widget_id, event_type, event_data) };
    }
  }
}

impl Default for LibraryLoader {
  fn default() -> Self {
    Self::new()
  }
}

impl Drop for LibraryLoader {
  fn drop(&mut self) {
    self.library = None;
  }
}
