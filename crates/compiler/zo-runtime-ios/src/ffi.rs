//! The `_zo_run_native` C-ABI entry point for iOS.

use zo_runtime_render::aot::{
  RebuildInputs, RegistryInputs, SendPtr, UpdateReport, ZoRuntimeContext,
  build_registry, decode_attr_bindings, decode_conditional_bindings,
  decode_list_bindings, decode_template, rebuild_with_regions,
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

  let base = match unsafe { decode_template(ctx_ref) } {
    Ok(c) => c,
    Err(e) => {
      eprintln!("[zo-runtime-ios] template decode error: {e:?}");

      return;
    }
  };

  let lists = unsafe { decode_list_bindings(ctx_ref) };
  let attrs = unsafe { decode_attr_bindings(ctx_ref) };
  let conditionals = unsafe { decode_conditional_bindings(ctx_ref) };

  // Initial frame: bake every `mut`'s value into its `Text`,
  // apply attribute bindings, and splice each list's initial
  // items over its placeholder. The postcard payload already
  // carries the scalar initials, so the text bake is a no-op on
  // the first frame; the splice brings a non-empty initial list
  // onto the screen.
  let initial = unsafe {
    rebuild_with_regions(RebuildInputs {
      base: &base,
      text_bindings_ptr: ctx_ref.text_bindings_ptr,
      text_bindings_count: ctx_ref.text_bindings_count,
      attrs: &attrs,
      lists: &lists,
      conditionals: &conditionals,
    })
  };

  let shared = Arc::new(Mutex::new(initial));
  let report = Arc::new(Mutex::new(UpdateReport::default()));

  // Build the registry only when the program registered a dispatcher;
  // a static template leaves it empty and taps no-op.
  let registry = match ctx_ref.handle_event {
    Some(dispatch) => build_registry(
      SendPtr(dispatch),
      Arc::clone(&shared),
      RegistryInputs {
        base,
        lists,
        attrs,
        bindings_ptr: SendPtr(ctx_ref.text_bindings_ptr),
        bindings_count: ctx_ref.text_bindings_count,
        report: Arc::clone(&report),
        conditionals,
      },
    ),
    None => EventRegistry::new(),
  };

  crate::app::install(registry, shared, report);
  crate::app::run();
}
