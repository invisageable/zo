use crate::scheduler;
use crate::task::TaskOutcome;

/// # Safety
///
/// Called from codegen on the `check` fail path.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_check_fail() {
  let in_task = scheduler::with(|s| s.current().is_some());

  if in_task {
    crate::task::exit_current_with_outcome(TaskOutcome::Panicked);
  } else {
    eprintln!("check failed");
    std::process::exit(1);
  }
}
