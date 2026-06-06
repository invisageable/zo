//! The `_zo_run_native` C-ABI entry point for iOS.

use zo_runtime_render::aot::{
  SendPtr, ZoRuntimeContext, build_registry, decode_template,
  refresh_bindings_from_global,
};
use zo_runtime_render::render::EventRegistry;

use std::sync::{Arc, Mutex};

/// AOT entry point. Decodes the embedded template, refreshes reactive
/// bindings into the initial command stream, builds the event
/// registry, hands both to the app delegate, then launches the UIKit
/// run loop (blocks until exit).
///
/// Exported as the Mach-O symbol `_zo_run_native` — the same symbol
/// the desktop runtime exports, so codegen's `BL _zo_run_native` is
/// target-independent; only the linked runtime differs.
///
/// # Safety
///
/// `ctx` must point to a valid `ZoRuntimeContext` that lives for the
/// duration of the call. The exe's stack frame outlives it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_run_native(ctx: *const ZoRuntimeContext) {
  if ctx.is_null() {
    eprintln!("[zo-runtime-ios] _zo_run_native: null context");

    return;
  }

  let ctx_ref = unsafe { &*ctx };

  let mut cmds = match unsafe { decode_template(ctx_ref) } {
    Ok(c) => c,
    Err(e) => {
      eprintln!("[zo-runtime-ios] template decode error: {e:?}");

      return;
    }
  };

  // Bake initial reactive values into the command stream. The
  // postcard payload already carries every `mut`'s initial value, so
  // this is a no-op on the first frame — it keeps the
  // refresh-after-tap path identical to startup.
  unsafe {
    refresh_bindings_from_global(
      ctx_ref.text_bindings_ptr,
      ctx_ref.text_bindings_count,
      &mut cmds,
    );
  }

  let shared = Arc::new(Mutex::new(cmds.clone()));

  // Build the registry only when the program registered a dispatcher;
  // a static template leaves it empty and taps no-op.
  let registry = match ctx_ref.handle_event {
    Some(dispatch) => build_registry(
      &cmds,
      SendPtr(dispatch),
      SendPtr(ctx_ref.text_bindings_ptr),
      ctx_ref.text_bindings_count,
      Arc::clone(&shared),
    ),
    None => EventRegistry::new(),
  };

  crate::app::install(registry, shared);
  crate::app::run();
}
