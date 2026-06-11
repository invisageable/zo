//! Desktop (eframe) C-ABI entry point for AOT-compiled zo programs.
//!
//! The platform-agnostic ABI + reactive plumbing — `ZoRuntimeContext`,
//! the `_zo_state_*` buffer, postcard decode, `TextBinding` refresh,
//! and the `EventRegistry` builder — lives in `zo_runtime_render::aot`
//! and is shared with every other backend (iOS, Android). This file is
//! just `_zo_run_native`: decode through `aot`, build the registry,
//! and drive the eframe `Runtime`. Blocks until the user closes the
//! window.

use crate::runtime::Runtime;

use zo_runtime_render::aot::{
  RegistryInputs, SendPtr, UpdateReport, ZoRuntimeContext, build_registry,
  decode_attr_bindings, decode_list_bindings, decode_template,
  rebuild_with_lists,
};
use zo_runtime_render::render::RuntimeConfig;

use std::sync::{Arc, Mutex};

/// AOT entry point. Decodes the initial template, refreshes
/// reactive bindings into the initial command stream, builds
/// the event registry, launches eframe. Blocks until the
/// window closes.
///
/// Exported as the Mach-O / ELF symbol `_zo_run_native` (Rust
/// source has no leading underscore — the C ABI prepends one on
/// Apple platforms; on Linux the bare name `zo_run_native` is
/// what the linker sees).
///
/// # Safety
///
/// `ctx` must point to a valid `ZoRuntimeContext` that lives
/// for the duration of the call. The exe's stack frame
/// outlives every runtime call, so the pointer is stable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_run_native(ctx: *const ZoRuntimeContext) {
  if ctx.is_null() {
    eprintln!("[zo-runtime] _zo_run_native: null context");

    return;
  }

  let ctx_ref = unsafe { &*ctx };

  let base = match unsafe { decode_template(ctx_ref) } {
    Ok(c) => c,
    Err(e) => {
      eprintln!("[zo-runtime] template decode error: {e:?}");

      return;
    }
  };

  let lists = unsafe { decode_list_bindings(ctx_ref) };
  let attrs = unsafe { decode_attr_bindings(ctx_ref) };

  // Initial frame: bake every `mut`'s value into its `Text`,
  // apply attribute bindings, and splice each list's initial
  // items over its placeholder. The postcard payload already
  // carries the scalar initials, so the text bake is a no-op on
  // the first frame; the splice brings a non-empty initial list
  // onto the screen.
  let initial = unsafe {
    rebuild_with_lists(
      &base,
      ctx_ref.text_bindings_ptr,
      ctx_ref.text_bindings_count,
      &attrs,
      &lists,
    )
  };

  let shared = Arc::new(Mutex::new(initial));
  let mut runtime = Runtime::with_config(RuntimeConfig::default());

  runtime.set_shared_commands(Arc::clone(&shared));

  if let Some(dispatch) = ctx_ref.handle_event {
    runtime.set_events(build_registry(
      SendPtr(dispatch),
      shared,
      RegistryInputs {
        base,
        lists,
        attrs,
        bindings_ptr: SendPtr(ctx_ref.text_bindings_ptr),
        bindings_count: ctx_ref.text_bindings_count,
        report: Arc::new(Mutex::new(UpdateReport::default())),
      },
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
  // after `#render`.
  std::process::exit(if status.is_ok() { 0 } else { 1 });
}
