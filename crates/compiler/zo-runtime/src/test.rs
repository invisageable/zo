use crate::scheduler;
use crate::task::{TaskOutcome, spawn};

use std::sync::atomic::{AtomicU32, Ordering};

static PASSED: AtomicU32 = AtomicU32::new(0);
static FAILED: AtomicU32 = AtomicU32::new(0);

/// # Safety
///
/// Called from synthesized test harness machine code.
#[unsafe(export_name = "zo_test_begin")]
pub unsafe extern "C-unwind" fn _zo_test_begin(count: u64) {
  eprintln!("\nrunning {count} tests ...\n");
}

/// # Safety
///
/// `fn_ptr` must be a live function address. `name_ptr`
/// must point to `name_len` valid UTF-8 bytes.
#[unsafe(export_name = "zo_test_run_one")]
pub unsafe extern "C-unwind" fn _zo_test_run_one(
  fn_ptr: extern "C-unwind" fn(),
  name_ptr: *const u8,
  name_len: u64,
) {
  let name = unsafe {
    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
      name_ptr,
      name_len as usize,
    ))
  };

  let task_handle = unsafe { spawn(fn_ptr) };

  unsafe {
    scheduler::drain_until_dead(task_handle);
  }

  let outcome = unsafe { (*task_handle).outcome };

  match outcome {
    TaskOutcome::Completed => {
      eprintln!("[PASS] {name}");
      PASSED.fetch_add(1, Ordering::Relaxed);
    }
    TaskOutcome::Panicked => {
      eprintln!("[FAIL] {name}");
      FAILED.fetch_add(1, Ordering::Relaxed);
    }
    TaskOutcome::Running => unreachable!(),
  }

  // Free the task without propagating panics.
  unsafe { drop(Box::from_raw(task_handle)) };
}

/// # Safety
///
/// Called from synthesized test harness machine code.
#[unsafe(export_name = "zo_test_summary")]
pub unsafe extern "C-unwind" fn _zo_test_summary() {
  let passed = PASSED.load(Ordering::Relaxed);
  let failed = FAILED.load(Ordering::Relaxed);

  eprintln!();

  if failed == 0 {
    eprintln!("test result: ok. {passed} passed, 0 failed.");
  } else {
    eprintln!(
      "test result: FAILED. {passed} passed, \
       {failed} failed."
    );
    std::process::exit(1);
  }
}
