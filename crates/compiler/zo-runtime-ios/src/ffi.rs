//! The `_zo_run_native` C-ABI entry point for iOS.

use zo_runtime_render::aot::{
  ZoRuntimeContext, decode_template, refresh_bindings_from_global,
};
use zo_ui_protocol::UiCommand;

use std::sync::{Mutex, OnceLock};

/// The decoded template the app delegate renders.
///
/// `UIApplicationMain` takes a delegate CLASS, not an instance, and
/// constructs the delegate itself — so the commands cannot be handed
/// in through a constructor. They reach the delegate through this
/// process global, set before `UIApplicationMain` spins up the run
/// loop and read once in `didFinishLaunchingWithOptions`.
static COMMANDS: OnceLock<Mutex<Vec<UiCommand>>> = OnceLock::new();

/// The template commands captured by `zo_run_native`, or empty if the
/// entry point hasn't run (defensive — the delegate then renders an
/// empty screen rather than panicking).
pub(crate) fn commands() -> Vec<UiCommand> {
  COMMANDS
    .get()
    .map(|m| m.lock().unwrap().clone())
    .unwrap_or_default()
}

/// AOT entry point. Decodes the embedded template, refreshes reactive
/// bindings into the initial command stream, stashes it for the
/// delegate, then launches the UIKit run loop (blocks until exit).
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

  unsafe {
    refresh_bindings_from_global(
      ctx_ref.text_bindings_ptr,
      ctx_ref.text_bindings_count,
      &mut cmds,
    );
  }

  let _ = COMMANDS.set(Mutex::new(cmds));

  crate::app::run();
}
