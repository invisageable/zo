//! Webview (wry) C-ABI entry point for AOT-compiled zo programs.
//!
//! Mirrors `zo-runtime-native`'s `_zo_run_native`, but drives the wry
//! webview `Runtime` instead of eframe. The platform-agnostic ABI +
//! reactive plumbing — `ZoRuntimeContext`, postcard decode, binding
//! refresh, the `EventRegistry` builder — lives in
//! `zo_runtime_render::aot` and is shared with every backend. This file
//! is just `_zo_run_web`: decode, build the registry, run the webview.
//! Blocks until the window closes.

use crate::runtime::Runtime;

use zo_runtime_render::aot::{
  RegistryInputs, SendPtr, UpdateReport, ZoRuntimeContext, build_registry,
  decode_attr_bindings, decode_list_bindings, decode_template,
  rebuild_with_lists,
};
use zo_runtime_render::render::RuntimeConfig;

use std::sync::{Arc, Mutex};

/// AOT entry point for `zo build --target webview`. Decodes the initial
/// template, refreshes reactive bindings into the initial command
/// stream, builds the event registry, launches the wry webview. Blocks
/// until the window closes.
///
/// Exported as the Mach-O / ELF symbol `_zo_run_web` (Rust source has
/// no leading underscore — the C ABI prepends one on Apple platforms).
/// The arm64 codegen emits a call to this symbol in place of
/// `_zo_run_native` when the program is built for the webview target.
///
/// # Safety
///
/// `ctx` must point to a valid `ZoRuntimeContext` that lives for the
/// duration of the call. The exe's stack frame outlives every runtime
/// call, so the pointer is stable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_run_web(ctx: *const ZoRuntimeContext) {
  if ctx.is_null() {
    eprintln!("[zo-runtime] _zo_run_web: null context");

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

  // Initial frame: bake every `mut`'s value into its `Text`, apply
  // attribute bindings, and splice each list's initial items over its
  // placeholder — identical to the native entry, since the command
  // stream is backend-agnostic.
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

  // The webview event loop returns only on window close. Exit on the
  // user's behalf rather than falling through to `main`'s tail —
  // nothing user-observable runs after `#render`, and wry/winit leave
  // background threads (webview process, wgpu) that would otherwise
  // keep the process alive.
  std::process::exit(if status.is_ok() { 0 } else { 1 });
}
