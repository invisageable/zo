//! C ABI for AOT-compiled zo programs.
//!
//! `_zo_run_native` is the single entry point: takes a
//! `ZoRuntimeContext` describing the initial template +
//! optional event/state callbacks, decodes the embedded
//! postcard payload, builds an `EventRegistry` whose
//! callbacks fire through `ctx.handle_event` and refresh
//! reactive `Text` bindings from the runtime-side state
//! buffer, then launches the eframe runtime. Blocks until
//! the user closes the window.

use crate::runtime::Runtime;

use zo_runtime_render::render::{EventHandler, EventRegistry, RuntimeConfig};
use zo_ui_protocol::UiCommand;
use zo_ui_protocol::codec::{self, CodecError};

use std::collections::HashSet;
use std::slice;
use std::sync::{Arc, Mutex, OnceLock};

/// The global reactive state, one `Vec<i64>` per program.
static STATE: OnceLock<Mutex<Vec<i64>>> = OnceLock::new();

/// The string-typed reactive state.
static STR_STATE: OnceLock<Mutex<Vec<Vec<u8>>>> = OnceLock::new();

fn state() -> &'static Mutex<Vec<i64>> {
  STATE.get_or_init(|| Mutex::new(Vec::new()))
}

fn str_state() -> &'static Mutex<Vec<Vec<u8>>> {
  STR_STATE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Encode `bytes` into `[len: u64][bytes][null]` layout — mirrors
/// `Insn::ConstString` so the result is a drop-in replacement for a static
/// string literal.
fn encode_length_prefixed(bytes: &[u8]) -> Vec<u8> {
  let len = bytes.len() as u64;
  let mut buf = Vec::with_capacity(8 + bytes.len() + 1);

  buf.extend_from_slice(&len.to_le_bytes());
  buf.extend_from_slice(bytes);
  buf.push(0);
  buf
}

/// Read the length-prefix at `ptr` and return the borrowed
/// bytes (without the trailing nul). Caller guarantees
/// `ptr` points to a `[len: u64][bytes][null]` buffer.
unsafe fn read_length_prefixed(ptr: *const u8) -> &'static [u8] {
  let len = unsafe { (ptr as *const u64).read_unaligned() } as usize;

  unsafe { slice::from_raw_parts(ptr.add(8), len) }
}

/// One reactive text binding: replace `commands[cmd_idx]`
/// (a `Text`) with the rendered form of `STATE[slot_id]`
/// (`is_str == 0`) or `STR_STATE[slot_id]` (`is_str != 0`).
/// `_pad` reserves a future tag byte without an ABI break.
#[repr(C)]
pub struct TextBinding {
  pub cmd_idx: u32,
  pub slot_id: u32,
  pub is_str: u32,
  pub _pad: u32,
}

/// AOT entry-point context. Built in the compiled exe's
/// stack frame by codegen and passed by reference to
/// `_zo_run_native` at startup.
///
/// Field order is part of the ABI. Optional callback /
/// pointer fields stay null when the program doesn't use
/// the matching feature. New fields may be appended
/// without breaking compiled binaries.
///
/// Reactive state lives **inside the runtime** (a global
/// `Vec<i64>` allocated by `zo_state_init`). Closures
/// mutate it through `zo_state_get` / `zo_state_set` FFI
/// calls — so the context only needs to know about the
/// per-program text-binding map; the state pointer never
/// crosses the ABI.
#[repr(C)]
pub struct ZoRuntimeContext {
  /// Pointer to postcard-encoded `Vec<UiCommand>` (in the
  /// exe's rodata).
  pub template_ptr: *const u8,
  /// Length in bytes of the template payload.
  pub template_len: usize,
  /// Called when a UI event fires. Null = static template
  /// (events silently ignored). `value_ptr` carries the
  /// payload as a zo-format length-prefixed string for
  /// text-bearing events (`@input`/`@change`/`@submit`),
  /// null for click. The pointer is valid for the call
  /// only; persist via `zo_state_set_str` (which copies).
  pub handle_event: Option<
    unsafe extern "C" fn(widget_id: u32, event_kind: u32, value_ptr: *const u8),
  >,
  /// Pointer to an array of `TextBinding` records that
  /// tell the runtime which `commands[cmd_idx]` to refresh
  /// from which `state[slot_id]`. Null = no reactive
  /// bindings (display never updates).
  pub text_bindings_ptr: *const TextBinding,
  /// Number of `TextBinding`s at `text_bindings_ptr`.
  pub text_bindings_count: usize,
}

/// Send wrapper for a raw pointer or fn pointer. The exe's
/// stack outlives the runtime call, so the pointers stay
/// valid for the whole `eframe::run_native` lifetime; the
/// runtime promises not to retain them past the call.
#[derive(Clone, Copy)]
struct SendPtr<T>(T);

unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

/// Decode the context's embedded template payload into a
/// command vec. Extracted so unit tests can exercise the
/// decode path without launching eframe.
///
/// Safety: the `(template_ptr, template_len)` pair must
/// describe a valid postcard-encoded `Vec<UiCommand>` byte
/// range owned by the caller for the duration of this call.
unsafe fn decode_template(
  ctx: &ZoRuntimeContext,
) -> Result<Vec<UiCommand>, CodecError> {
  let bytes =
    unsafe { slice::from_raw_parts(ctx.template_ptr, ctx.template_len) };

  codec::decode(bytes)
}

/// Resize the global state buffers to `count` slots
/// (idempotent, grow-only). `STR_STATE` is sized in
/// lockstep so an early `zo_state_get_str` returns a
/// valid empty-string pointer rather than crashing.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_init(count: u32) {
  let mut state = state().lock().unwrap();

  if state.len() < count as usize {
    state.resize(count as usize, 0);
  }

  let mut str_state = str_state().lock().unwrap();

  if str_state.len() < count as usize {
    str_state.resize_with(count as usize, || encode_length_prefixed(b""));
  }
}

/// Read state slot `slot`. Returns 0 for out-of-range
/// slots (defensive — codegen should never emit a get
/// outside the `init`-declared range, but a stale binary
/// against a newer runtime should fail soft, not crash).
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_get(slot: u32) -> i64 {
  let state = state().lock().unwrap();

  state.get(slot as usize).copied().unwrap_or(0)
}

/// Write `value` into state slot `slot`. Out-of-range
/// writes are silently dropped (same defensive rationale
/// as `zo_state_get`).
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_set(slot: u32, value: i64) {
  let mut state = state().lock().unwrap();

  if let Some(s) = state.get_mut(slot as usize) {
    *s = value;
  }
}

/// Copy the length-prefixed string at `ptr` into the
/// reactive string slot. The closure body (or main's
/// initialiser) passes the zo-internal `str` pointer
/// directly — the runtime owns the resulting bytes so
/// the buffer stays valid past the closure's frame.
///
/// Safety: `ptr` must be either null (silently treated as
/// the empty string) or a zo-format length-prefixed
/// string pointer (`[len: u64][bytes][null]`) that lives
/// for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_state_set_str(slot: u32, ptr: *const u8) {
  let bytes: &[u8] = if ptr.is_null() {
    b""
  } else {
    unsafe { read_length_prefixed(ptr) }
  };

  let encoded = encode_length_prefixed(bytes);
  let mut str_state = str_state().lock().unwrap();

  if let Some(slot_buf) = str_state.get_mut(slot as usize) {
    *slot_buf = encoded;
  }
}

/// Return a pointer to `slot`'s length-prefixed bytes.
/// The pointer dangles only if a subsequent
/// `zo_state_set_str` overwrites the same slot — same
/// hazard the Rust borrow checker would reject for
/// `let r = &v; v = ...; *r`. Out-of-range reads return a
/// static empty length-prefixed buffer (never null), so
/// codegen can unconditionally dereference.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_get_str(slot: u32) -> *const u8 {
  // Matches `encode_length_prefixed(b"")` byte-for-byte;
  // `empty_static_matches_encoded_empty` enforces lockstep.
  static EMPTY: [u8; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 0];

  let str_state = str_state().lock().unwrap();

  match str_state.get(slot as usize) {
    Some(buf) => buf.as_ptr(),
    None => EMPTY.as_ptr(),
  }
}

/// Refresh every reactive `commands[cmd_idx]` from
/// `state` / `str_state`. Pure inner that takes state
/// explicitly — `refresh_bindings_from_global` wraps it
/// for production while tests inject deterministic state.
///
/// Safety: `bindings_ptr` must point to `bindings_count`
/// valid `TextBinding` entries.
unsafe fn refresh_bindings(
  state: &[i64],
  str_state: &[Vec<u8>],
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  if bindings_ptr.is_null() {
    return;
  }

  let bindings = unsafe { slice::from_raw_parts(bindings_ptr, bindings_count) };

  for binding in bindings {
    let slot = binding.slot_id as usize;
    let cmd_idx = binding.cmd_idx as usize;

    if cmd_idx >= commands.len() {
      continue;
    }

    let UiCommand::Text(target) = &mut commands[cmd_idx] else {
      continue;
    };

    if binding.is_str != 0 {
      let buf = match str_state.get(slot) {
        Some(b) if b.len() >= 8 => b,
        _ => continue,
      };
      let len = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;

      if 8 + len > buf.len() {
        continue;
      }

      let new_bytes = &buf[8..8 + len];

      // Bytes-equal short-circuit avoids the
      // `String::from_utf8_lossy` allocation on the no-op
      // path — for steady-state UI loops refresh fires
      // every frame and most slots are unchanged.
      if target.as_bytes() == new_bytes {
        continue;
      }

      *target = String::from_utf8_lossy(new_bytes).into_owned();
    } else {
      let value = match state.get(slot) {
        Some(v) => *v,
        None => continue,
      };
      let new_text = value.to_string();

      if *target != new_text {
        *target = new_text;
      }
    }
  }
}

/// Production wrapper — holds both reactive-state mutexes
/// across the binding walk and forwards to
/// `refresh_bindings`.
unsafe fn refresh_bindings_from_global(
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  let state = state().lock().unwrap();
  let str_state = str_state().lock().unwrap();

  unsafe {
    refresh_bindings(&state, &str_state, bindings_ptr, bindings_count, commands)
  };
}

/// Build an `EventRegistry` whose callbacks dispatch
/// through `ctx.handle_event` AND, after the dispatcher
/// returns, refresh reactive bindings from the global
/// state buffer. Walks `cmds` left-to-right, assigns each
/// unique handler name a sequential `u32` index — the
/// codegen-side dispatcher uses the same dedupe-by-first-
/// seen-name scheme, so the indices line up automatically.
fn build_registry(
  cmds: &[UiCommand],
  dispatch: SendPtr<unsafe extern "C" fn(u32, u32, *const u8)>,
  bindings_ptr: SendPtr<*const TextBinding>,
  bindings_count: usize,
  shared_cmds: Arc<Mutex<Vec<UiCommand>>>,
) -> EventRegistry {
  let mut registry = EventRegistry::new();
  let mut seen: HashSet<String> = HashSet::new();
  let mut handler_idx: u32 = 0;

  for cmd in cmds {
    let UiCommand::Event {
      handler,
      event_kind,
      ..
    } = cmd
    else {
      continue;
    };

    if !seen.insert(handler.clone()) {
      continue;
    }

    let idx = handler_idx;
    let kind_u32 = *event_kind as u32;
    // Capture `SendPtr` instances directly into the closure
    // — the wrapper is Send, the bare raw pointers / fn
    // pointers aren't.
    let dispatch_send = dispatch;
    let bindings_send = bindings_ptr;
    let cmds_arc = Arc::clone(&shared_cmds);
    let cb: EventHandler = Box::new(move |payload| {
      // RFC 2229 disjoint captures would otherwise pull
      // `dispatch_send.0` / `bindings_send.0` directly
      // into the closure type, defeating `SendPtr`'s
      // wrapper-level `Send`. Reference each binding
      // whole to force whole-binding captures.
      let _ = (&dispatch_send, &bindings_send);

      // `event_holder` IS the `Event { value: str }` struct
      // — one 8-byte field holding a pointer to the
      // length-prefixed payload buffer. Both live on this
      // stack frame for the call's duration; the closure
      // copies via `zo_state_set_str` if it wants to keep
      // the value past the call.
      let payload_buf = encode_length_prefixed(payload.value.as_bytes());
      let event_holder: u64 = payload_buf.as_ptr() as u64;

      // Dispatcher's `mov x1, x2` forwards `&event_holder`
      // into the closure's `param[1]`, then tail-calls.
      unsafe {
        (dispatch_send.0)(
          idx,
          kind_u32,
          &event_holder as *const u64 as *const u8,
        )
      };

      let mut cmds = cmds_arc.lock().unwrap();

      unsafe {
        refresh_bindings_from_global(
          bindings_send.0,
          bindings_count,
          &mut cmds,
        );
      }
    });

    registry.register(handler.clone(), cb);
    handler_idx += 1;
  }

  registry
}

/// AOT entry point. Decodes the initial template, refreshes
/// reactive bindings into the initial command stream, builds
/// the event registry, launches eframe. Blocks until the
/// window closes.
///
/// Exported as the Mach-O / ELF symbol `_zo_run_native`
/// (Rust source has no leading underscore — the C ABI
/// prepends one on Apple platforms; on Linux the bare name
/// `zo_run_native` is what the linker sees).
///
/// Safety: `ctx` must point to a valid `ZoRuntimeContext`
/// that lives for the duration of the call. The exe's stack
/// frame outlives every runtime call, so the pointer is
/// stable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_run_native(ctx: *const ZoRuntimeContext) {
  if ctx.is_null() {
    eprintln!("[zo-runtime] _zo_run_native: null context");

    return;
  }

  let ctx_ref = unsafe { &*ctx };

  let mut cmds = match unsafe { decode_template(ctx_ref) } {
    Ok(c) => c,
    Err(e) => {
      eprintln!("[zo-runtime] template decode error: {e:?}");

      return;
    }
  };

  // Initial refresh — the postcard payload bakes the
  // initial value of every `mut` into its `Text`, so this
  // is a no-op on the first frame, but keeps the
  // refresh-after-event path simple (always go through the
  // same code).
  unsafe {
    refresh_bindings_from_global(
      ctx_ref.text_bindings_ptr,
      ctx_ref.text_bindings_count,
      &mut cmds,
    );
  }

  let shared = Arc::new(Mutex::new(cmds.clone()));
  let mut runtime = Runtime::with_config(RuntimeConfig::default());

  runtime.set_shared_commands(Arc::clone(&shared));

  if let Some(dispatch) = ctx_ref.handle_event {
    runtime.set_events(build_registry(
      &cmds,
      SendPtr(dispatch),
      SendPtr(ctx_ref.text_bindings_ptr),
      ctx_ref.text_bindings_count,
      shared,
    ));
  }

  let status = runtime.run();

  if let Err(e) = &status {
    eprintln!("[zo-runtime] runtime error: {e}");
  }

  // The runtime returns only on window close. eframe's
  // `NSApp.run()` returns cleanly on macOS but doesn't
  // teardown background threads (image loader worker,
  // wgpu device threads, Cocoa autorelease state), so
  // letting `main` fall through here leaves the process
  // alive — the user has to Ctrl+C the terminal. Exit
  // immediately on the user's behalf; nothing
  // user-observable depends on running `main`'s tail
  // after `#dom`.
  std::process::exit(if status.is_ok() { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
  use super::*;
  use zo_ui_protocol::{ElementTag, EventKind, UiCommand};

  fn empty_ctx_with_template(bytes: *const u8, len: usize) -> ZoRuntimeContext {
    ZoRuntimeContext {
      template_ptr: bytes,
      template_len: len,
      handle_event: None,
      text_bindings_ptr: std::ptr::null(),
      text_bindings_count: 0,
    }
  }

  #[test]
  fn empty_static_matches_encoded_empty() {
    // The out-of-range fallback in `zo_state_get_str` is
    // hand-rolled (`[u8; 9]` of zeros) instead of going
    // through `encode_length_prefixed(b"")` so the function
    // can return a `'static` pointer without `OnceLock`.
    // Lockstep with the encoder via this assertion — if
    // the prefix layout ever changes (e.g. 4-byte length)
    // the static silently desyncs without it.
    let encoded = encode_length_prefixed(b"");

    let fallback = unsafe {
      slice::from_raw_parts(zo_state_get_str(u32::MAX), encoded.len())
    };

    assert_eq!(fallback, encoded.as_slice());
  }

  #[test]
  fn text_binding_layout_matches_codegen_pack() {
    // Codegen serialises 16 bytes per entry: cmd_idx@0,
    // slot_id@4, is_str@8, _pad@12. If this drifts, the
    // binding pointer the runtime decodes will mis-align
    // and the wrong slot/flag will land per binding.
    use std::mem::{align_of, offset_of, size_of};

    assert_eq!(size_of::<TextBinding>(), 16);
    assert_eq!(align_of::<TextBinding>(), 4);
    assert_eq!(offset_of!(TextBinding, cmd_idx), 0);
    assert_eq!(offset_of!(TextBinding, slot_id), 4);
    assert_eq!(offset_of!(TextBinding, is_str), 8);
    assert_eq!(offset_of!(TextBinding, _pad), 12);
  }

  #[test]
  fn decode_template_round_trip() {
    let cmds = vec![
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("hello".into()),
      UiCommand::EndElement,
    ];

    let bytes = codec::encode(&cmds).unwrap();
    let ctx = empty_ctx_with_template(bytes.as_ptr(), bytes.len());

    let decoded = unsafe { decode_template(&ctx) }.unwrap();

    assert_eq!(decoded, cmds);
  }

  #[test]
  fn decode_template_rejects_garbage() {
    // Random bytes don't encode any command stream.
    let garbage = [0xFFu8, 0xFE, 0xFD, 0xFC];
    let ctx = empty_ctx_with_template(garbage.as_ptr(), garbage.len());

    assert!(unsafe { decode_template(&ctx) }.is_err());
  }

  #[test]
  fn refresh_bindings_replaces_text_from_state() {
    let state = [42i64];
    let str_state: Vec<Vec<u8>> = vec![vec![]];
    let mut cmds = vec![
      UiCommand::Text("0".into()),
      UiCommand::Text("ignored".into()),
    ];
    let bindings = [TextBinding {
      cmd_idx: 0,
      slot_id: 0,
      is_str: 0,
      _pad: 0,
    }];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    assert_eq!(cmds[0], UiCommand::Text("42".into()));
    // Untouched binding leaves the second Text alone.
    assert_eq!(cmds[1], UiCommand::Text("ignored".into()));
  }

  #[test]
  fn refresh_bindings_replaces_text_from_str_state() {
    let state: [i64; 1] = [0];
    let str_state: Vec<Vec<u8>> = vec![encode_length_prefixed(b"hello")];
    let mut cmds = vec![UiCommand::Text("".into())];
    let bindings = [TextBinding {
      cmd_idx: 0,
      slot_id: 0,
      is_str: 1,
      _pad: 0,
    }];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    assert_eq!(cmds[0], UiCommand::Text("hello".into()));
  }

  #[test]
  fn refresh_bindings_handles_null_pointers() {
    let state: [i64; 0] = [];
    let str_state: Vec<Vec<u8>> = vec![];
    let mut cmds = vec![UiCommand::Text("safe".into())];

    unsafe {
      refresh_bindings(&state, &str_state, std::ptr::null(), 0, &mut cmds)
    };

    assert_eq!(cmds[0], UiCommand::Text("safe".into()));
  }

  #[test]
  fn refresh_bindings_skips_oob_indices() {
    let state = [1i64];
    let str_state: Vec<Vec<u8>> = vec![vec![]];
    let mut cmds = vec![UiCommand::Text("only".into())];
    let bindings = [
      // Out-of-range cmd_idx — should be skipped.
      TextBinding {
        cmd_idx: 5,
        slot_id: 0,
        is_str: 0,
        _pad: 0,
      },
      // Out-of-range slot_id — should be skipped.
      TextBinding {
        cmd_idx: 0,
        slot_id: 10,
        is_str: 0,
        _pad: 0,
      },
    ];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    // Neither binding applied; original content preserved.
    assert_eq!(cmds[0], UiCommand::Text("only".into()));
  }

  #[test]
  fn state_set_get_str_round_trip() {
    zo_state_init(202);

    let payload = encode_length_prefixed(b"world");

    unsafe { zo_state_set_str(201, payload.as_ptr()) };

    let p = zo_state_get_str(201);

    let bytes = unsafe { read_length_prefixed(p) };

    assert_eq!(bytes, b"world");
    // Out-of-range reads return the static empty buffer
    // (length 0) rather than null/segfaulting.
    let q = zo_state_get_str(99999);
    let bytes = unsafe { read_length_prefixed(q) };

    assert_eq!(bytes, b"");
  }

  #[test]
  fn state_get_set_round_trip() {
    // Slot 100 to avoid contention with other tests
    // using low slots (cargo runs tests in parallel and
    // STATE is a process-global `OnceLock<Mutex<Vec>>`).
    zo_state_init(101);
    zo_state_set(100, 99);

    assert_eq!(zo_state_get(100), 99);
    // Out-of-range reads return 0, never panic.
    assert_eq!(zo_state_get(9999), 0);
  }

  // --- dispatcher index assignment ---
  //
  // We can't easily test the dispatcher itself (cross-thread
  // fn-ptr call without a real eframe runtime), but we CAN
  // verify the index-assignment logic that has to mirror
  // codegen's dedupe scheme. If these diverge the dispatcher
  // routes events to wrong handlers.

  unsafe extern "C" fn noop_dispatch(
    _idx: u32,
    _kind: u32,
    _value_ptr: *const u8,
  ) {
  }

  fn registered_handlers(cmds: &[UiCommand]) -> Vec<String> {
    let registry = build_registry(
      cmds,
      SendPtr(noop_dispatch as _),
      SendPtr(std::ptr::null()),
      0,
      Arc::new(Mutex::new(vec![])),
    );
    let mut out: Vec<String> = cmds
      .iter()
      .filter_map(|c| match c {
        UiCommand::Event { handler, .. } => {
          registry.has(handler).then_some(handler.clone())
        }
        _ => None,
      })
      .collect();

    out.sort();
    out.dedup();
    out
  }

  #[test]
  fn build_registry_dedupes_handlers() {
    // Same handler on two widgets = one registry entry.
    let cmds = vec![
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
      UiCommand::Event {
        widget_id: "2".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
    ];

    let names = registered_handlers(&cmds);

    assert_eq!(names, vec!["__closure_0".to_string()]);
  }

  #[test]
  fn build_registry_distinct_handlers() {
    let cmds = vec![
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
      UiCommand::Event {
        widget_id: "2".into(),
        event_kind: EventKind::Click,
        handler: "__closure_1".into(),
      },
    ];

    let names = registered_handlers(&cmds);

    assert_eq!(
      names,
      vec!["__closure_0".to_string(), "__closure_1".to_string()]
    );
  }
}
