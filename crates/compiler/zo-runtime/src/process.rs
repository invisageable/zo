//! Subprocess execution — `core/process.zo` FFI backing.
//!
//! Uses `std::process::Command` for portability. Returns
//! a packed byte buffer with exit code + captured streams.

use crate::str::{alloc_str, str_bytes};

/// Execute a program with arguments and capture output.
///
/// `program` is a zo str (length-prefixed). `argv` is a
/// zo `[]str` array (length-prefixed pointer array).
/// `capture` selects which streams to capture:
///   0 = inherit (no capture)
///   1 = capture stdout
///   2 = capture stderr
///   3 = capture both
///
/// Returns a packed zo array:
///   `[exit_code_str, stdout_str, stderr_str]`
/// as a zo `[]str` pointer. On spawn failure returns a
/// 3-element array with exit code = -1 and empty strings.
///
/// # Safety
///
/// `program` and `argv` must be valid zo str / `[]str`
/// pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zo_process_exec(
  program: *const u8,
  argv: *const u8,
  capture: i32,
) -> *const u8 {
  let prog = unsafe { zo_str_to_rust(program) };
  let args = unsafe { zo_str_array_to_rust(argv) };

  let mut cmd = std::process::Command::new(&prog);

  cmd.args(&args);

  if capture & 1 != 0 {
    cmd.stdout(std::process::Stdio::piped());
  }

  if capture & 2 != 0 {
    cmd.stderr(std::process::Stdio::piped());
  }

  match cmd.output() {
    Ok(output) => {
      let code = output.status.code().unwrap_or(-1);
      let code_str = format!("{code}");

      let ptrs = [
        alloc_str(code_str.as_bytes()),
        alloc_str(&output.stdout),
        alloc_str(&output.stderr),
      ];

      crate::arr::alloc_ptr_array(&ptrs)
    }
    Err(_) => {
      let ptrs = [alloc_str(b"-1"), alloc_str(b""), alloc_str(b"")];

      crate::arr::alloc_ptr_array(&ptrs)
    }
  }
}

/// Convert a zo str pointer to a Rust `String`.
unsafe fn zo_str_to_rust(ptr: *const u8) -> String {
  if ptr.is_null() {
    return String::new();
  }

  let bytes = unsafe { str_bytes(ptr) };

  String::from_utf8_lossy(bytes).into_owned()
}

/// Convert a zo `[]str` array to `Vec<String>`.
///
/// zo array layout: `[len:u64][cap:u64][ptr0:u64]...`
unsafe fn zo_str_array_to_rust(arr: *const u8) -> Vec<String> {
  if arr.is_null() {
    return Vec::new();
  }

  let len = unsafe {
    u64::from_le_bytes(std::slice::from_raw_parts(arr, 8).try_into().unwrap())
      as usize
  };

  let mut result = Vec::with_capacity(len);

  for i in 0..len {
    let elem_ptr = unsafe {
      let offset = 16 + i * 8;
      let bytes = std::slice::from_raw_parts(arr.add(offset), 8);
      u64::from_le_bytes(bytes.try_into().unwrap()) as *const u8
    };

    result.push(unsafe { zo_str_to_rust(elem_ptr) });
  }

  result
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::str::alloc_str;

  #[test]
  fn echo_captures_stdout() {
    let program = alloc_str(b"echo");
    let arg = alloc_str(b"hello");
    let argv = crate::arr::alloc_ptr_array(&[arg]);

    let result = unsafe { zo_process_exec(program, argv, 3) };

    let len = unsafe {
      u64::from_le_bytes(
        std::slice::from_raw_parts(result, 8).try_into().unwrap(),
      ) as usize
    };

    assert_eq!(len, 3);

    let code_ptr = unsafe {
      let offset = 16;
      let bytes = std::slice::from_raw_parts(result.add(offset), 8);
      u64::from_le_bytes(bytes.try_into().unwrap()) as *const u8
    };

    let code_str = unsafe { str_bytes(code_ptr) };

    assert_eq!(code_str, b"0");

    let stdout_ptr = unsafe {
      let offset = 24;
      let bytes = std::slice::from_raw_parts(result.add(offset), 8);
      u64::from_le_bytes(bytes.try_into().unwrap()) as *const u8
    };

    let stdout = unsafe { str_bytes(stdout_ptr) };

    assert_eq!(std::str::from_utf8(stdout).unwrap().trim(), "hello");
  }

  #[test]
  fn nonexistent_program_returns_negative_code() {
    let program = alloc_str(b"/nonexistent_program_that_does_not_exist");
    let argv = crate::arr::alloc_ptr_array(&[]);

    let result = unsafe { zo_process_exec(program, argv, 3) };

    let code_ptr = unsafe {
      let offset = 16;
      let bytes = std::slice::from_raw_parts(result.add(offset), 8);
      u64::from_le_bytes(bytes.try_into().unwrap()) as *const u8
    };

    let code_str = unsafe { str_bytes(code_ptr) };

    assert_eq!(code_str, b"-1");
  }
}
