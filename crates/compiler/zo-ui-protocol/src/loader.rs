//! Dynamic library loader for zo applications.
//!
//! Parses the binary UiCommand array produced by
//! `zo-codegen-arm/src/codegen/template.rs` back into an
//! in-memory `Vec<UiCommand>`. The binary format for the new
//! unified Element model is a work in progress — for R1 the
//! decoder returns empty commands for the new type codes so
//! that the dylib path does not crash, without implementing the
//! full attribute encoding. The interactive `zo run` path
//! bypasses the loader entirely.

use crate::ui_protocol::UiCommand;

use libloading::{Library, Symbol};
use thin_vec::ThinVec;

use std::ffi::c_void;

/// The signature for the ui entry point function from the compiled zo library.
pub type UiEntryPoint = unsafe extern "C" fn() -> *mut c_void;
/// The signature for the event handler function from the compiled zo library.
pub type EventHandler = unsafe extern "C" fn(*mut c_void, u32, *mut c_void);

/// The offset of a command — 8 (after count + padding).
const COMMAND_START_OFFSET: usize = 8;
/// The size of a command — 16 bytes.
const COMMAND_SIZE: usize = 16;

/// Represents a UI command array as returned by the compiled program.
#[repr(C)]
struct UiCommandArray {
  count: u32,
  /// 4 bytes padding for alignment Commands follow immediately after at offset
  /// 8.
  _padding: u32,
}

/// Represents a single UI command in memory.
#[repr(C)]
struct RawUiCommand {
  command_type: u32,
  /// 4 bytes padding for 8-byte alignment.
  _padding: u32,
  /// Pointer to command-specific data.
  data: *mut c_void,
}

/// Loads and manages dynamic libraries compiled from zo programs
pub struct LibraryLoader {
  library: Option<Library>,
  event_handler: Option<Symbol<'static, EventHandler>>,
  base_address: *const u8,
}

impl LibraryLoader {
  /// Creates a new [`LibraryLooader`] instance.
  pub fn new() -> Self {
    Self {
      library: None,
      event_handler: None,
      base_address: std::ptr::null(),
    }
  }

  /// Loads a dynamic library and extract UI commands.
  pub fn load(
    &mut self,
    path: &str,
  ) -> Result<ThinVec<UiCommand>, Box<dyn std::error::Error>> {
    let lib = unsafe { Library::new(path) }?;

    let entry_point: Symbol<UiEntryPoint> =
      unsafe { lib.get(b"_zo_ui_entry_point") }?;

    // Try to get the event handler (optional)
    if let Ok(handler) =
      unsafe { lib.get::<Symbol<EventHandler>>(b"_zo_handle_event") }
    {
      // Leak the symbol to get 'static lifetime
      self.event_handler = Some(unsafe {
        std::mem::transmute::<
          Symbol<'_, Symbol<'_, EventHandler>>,
          Symbol<'static, EventHandler>,
        >(handler)
      });
    }

    // Call entry point to get command array
    let array_ptr = unsafe { entry_point() as *const UiCommandArray };

    if array_ptr.is_null() {
      return Ok(ThinVec::new());
    }

    // Store base address for offset resolution
    self.base_address = array_ptr as *const u8;

    // Parse commands
    let commands = self.parse_command_array(array_ptr);

    // Keep library loaded
    self.library = Some(lib);

    Ok(commands)
  }

  /// Parse the raw command array into UiCommand structs
  fn parse_command_array(
    &self,
    array_ptr: *const UiCommandArray,
  ) -> ThinVec<UiCommand> {
    let array = unsafe { &*array_ptr };
    let count = array.count as usize;
    let mut commands = ThinVec::with_capacity(count);

    let commands_base =
      unsafe { (array_ptr as *const u8).add(COMMAND_START_OFFSET) };

    for i in 0..count {
      let cmd_offset = i * COMMAND_SIZE;
      let cmd_ptr =
        unsafe { commands_base.add(cmd_offset) } as *const RawUiCommand;
      let raw_cmd = unsafe { &*cmd_ptr };
      let ui_cmd = self.parse_command(raw_cmd);

      if let Some(cmd) = ui_cmd {
        commands.push(cmd);
      }
    }

    commands
  }

  /// Parse a single command. The binary format for the
  /// unified Element model is not yet implemented — the
  /// decoder currently returns `None` for every type code.
  /// The AOT dylib path is a no-op for templates until the
  /// encoder/decoder pair gets redesigned to carry attributes.
  /// The interactive `zo run` path is unaffected.
  fn parse_command(&self, raw_cmd: &RawUiCommand) -> Option<UiCommand> {
    let _ = raw_cmd;
    None
  }

  /// Call the event handler if available.
  pub fn handle_event(
    &self,
    widget_id: *mut c_void,
    event_type: u32,
    event_data: *mut c_void,
  ) {
    if let Some(ref handler) = self.event_handler {
      unsafe {
        handler(widget_id, event_type, event_data);
      }
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
