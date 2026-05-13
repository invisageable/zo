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

/// Global reactive state — one `Vec<i64>` per program.
/// Allocated lazily by `zo_state_init` (called from the
/// AOT exe's `main` prologue when the program has reactive
/// bindings); read & written by `zo_state_get` /
/// `zo_state_set` and by `refresh_bindings` on the runtime
/// side.
///
/// `OnceLock` because `zo_state_init` runs once per
/// process before any other state access. The inner
/// `Mutex` serialises read/write — closures fire from
/// egui's UI thread, while the renderer's refresh runs
/// from the same thread immediately after, so contention
/// is nil; the lock is for soundness, not throughput.
static STATE: OnceLock<Mutex<Vec<i64>>> = OnceLock::new();

fn state() -> &'static Mutex<Vec<i64>> {
  STATE.get_or_init(|| Mutex::new(Vec::new()))
}

/// One reactive text binding emitted by codegen and read
/// by the runtime: replace `commands[cmd_idx]` (a `Text`)
/// with `Text(state[slot_id].to_string())` after every
/// event dispatch / state mutation.
#[repr(C)]
pub struct TextBinding {
  pub cmd_idx: u32,
  pub slot_id: u32,
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
  /// (events silently ignored).
  pub handle_event:
    Option<unsafe extern "C" fn(widget_id: u32, event_kind: u32)>,
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

/// Allocate (or grow) the global state buffer to `count`
/// `i64` slots, all zero-initialised. Idempotent — the
/// AOT exe calls this once at the top of `main`; further
/// calls only resize upward.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_init(count: u32) {
  let mut state = state().lock().unwrap();

  if state.len() < count as usize {
    state.resize(count as usize, 0);
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

/// Read every reactive `TextBinding` once and refresh the
/// matching `commands[cmd_idx]` from the supplied `state`
/// slice. Cheap to call on every event — only writes when
/// the rendered string actually differs from the current
/// `Text` content.
///
/// Pure inner that takes state explicitly — keeps the
/// production path (`refresh_bindings_from_global`) and
/// the unit tests on the same code path while letting
/// tests inject deterministic state without racing against
/// other tests through `STATE`.
///
/// Safety: `bindings_ptr` must refer to a valid array of
/// the length advertised in the context.
unsafe fn refresh_bindings(
  state: &[i64],
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  if bindings_ptr.is_null() {
    return;
  }

  let bindings =
    unsafe { slice::from_raw_parts(bindings_ptr, bindings_count) };

  for binding in bindings {
    let slot = binding.slot_id as usize;

    if slot >= state.len() {
      continue;
    }

    let cmd_idx = binding.cmd_idx as usize;

    if cmd_idx >= commands.len() {
      continue;
    }

    let new_text = state[slot].to_string();

    if let UiCommand::Text(s) = &mut commands[cmd_idx]
      && s != &new_text
    {
      *s = new_text;
    }
  }
}

/// Production wrapper — holds the `STATE` mutex across
/// the binding walk and forwards to `refresh_bindings`.
/// Lock hold-time is the walk itself; event-handler
/// dispatch and the renderer's refresh both run on the
/// same UI thread per the doc-comment on `STATE`, so no
/// contention.
unsafe fn refresh_bindings_from_global(
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  let state = state().lock().unwrap();

  unsafe {
    refresh_bindings(&state, bindings_ptr, bindings_count, commands)
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
  dispatch: SendPtr<unsafe extern "C" fn(u32, u32)>,
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
    let cb: EventHandler = Box::new(move |_payload| {
      // RFC 2229 disjoint captures would otherwise pull
      // `dispatch_send.0` / `bindings_send.0` directly
      // into the closure type, defeating `SendPtr`'s
      // wrapper-level `Send`. Reference each binding
      // whole to force whole-binding captures.
      let _ = (&dispatch_send, &bindings_send);

      // 1. Fire the exe's dispatcher. Tail-calls into the
      //    matching closure; closure mutates state via
      //    `zo_state_set` FFI calls.
      unsafe { (dispatch_send.0)(idx, kind_u32) };

      // 2. Refresh reactive `Text` bindings from the
      //    runtime-side state buffer. The renderer pulls
      //    from `shared_cmds` each frame, so this is
      //    enough to drive the display update on the
      //    next repaint.
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
    let mut cmds =
      vec![UiCommand::Text("0".into()), UiCommand::Text("ignored".into())];
    let bindings = [TextBinding {
      cmd_idx: 0,
      slot_id: 0,
    }];

    unsafe {
      refresh_bindings(&state, bindings.as_ptr(), bindings.len(), &mut cmds);
    }

    assert_eq!(cmds[0], UiCommand::Text("42".into()));
    // Untouched binding leaves the second Text alone.
    assert_eq!(cmds[1], UiCommand::Text("ignored".into()));
  }

  #[test]
  fn refresh_bindings_handles_null_pointers() {
    let state: [i64; 0] = [];
    let mut cmds = vec![UiCommand::Text("safe".into())];

    unsafe { refresh_bindings(&state, std::ptr::null(), 0, &mut cmds) };

    assert_eq!(cmds[0], UiCommand::Text("safe".into()));
  }

  #[test]
  fn refresh_bindings_skips_oob_indices() {
    let state = [1i64];
    let mut cmds = vec![UiCommand::Text("only".into())];
    let bindings = [
      // Out-of-range cmd_idx — should be skipped.
      TextBinding {
        cmd_idx: 5,
        slot_id: 0,
      },
      // Out-of-range slot_id — should be skipped.
      TextBinding {
        cmd_idx: 0,
        slot_id: 10,
      },
    ];

    unsafe {
      refresh_bindings(&state, bindings.as_ptr(), bindings.len(), &mut cmds);
    }

    // Neither binding applied; original content preserved.
    assert_eq!(cmds[0], UiCommand::Text("only".into()));
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

  unsafe extern "C" fn noop_dispatch(_idx: u32, _kind: u32) {}

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
