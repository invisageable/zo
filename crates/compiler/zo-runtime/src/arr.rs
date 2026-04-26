//! Runtime helpers for `[]T` arrays.
//!
//! `_zo_arr_sort_i32` sorts a `[]int` (default int = i32)
//! in place via Rust's stable slice sort. Mirrors the
//! pattern used by `zo_map_*` / `zo_vec_*` — the compiler
//! emits a direct `BL _zo_arr_sort_i32` from
//! `arr_int::sort`'s codegen handler with `X0` pointing
//! at the array's data and `X1` carrying the element
//! count.

/// Sort `[len]` ints starting at `data` in place.
///
/// zo's `[]int` codegen lays out each element as a 64-bit
/// slot (the element width matches the codegen's
/// register-sized load/store for indexed access),
/// regardless of the declared int width. Sort as `i64`
/// to match.
///
/// # Safety
///
/// `data` must be aligned for `i64`, non-null when
/// `len > 0`, and reference `len` valid 64-bit slots.
/// The zo codegen guarantees both when it passes
/// `(arr_ptr + 16, *(arr_ptr + 0))`.
#[unsafe(export_name = "zo_arr_sort_i32")]
pub unsafe extern "C-unwind" fn _zo_arr_sort_i32(data: *mut i64, len: usize) {
  if len == 0 {
    return;
  }

  let slice = unsafe { std::slice::from_raw_parts_mut(data, len) };

  slice.sort_unstable();
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_no_op() {
    let mut buf: [i64; 0] = [];

    unsafe {
      _zo_arr_sort_i32(buf.as_mut_ptr(), 0);
    }
  }

  #[test]
  fn sorts_in_place() {
    let mut buf: [i64; 8] = [3, 1, 4, 1, 5, 9, 2, 6];

    unsafe {
      _zo_arr_sort_i32(buf.as_mut_ptr(), buf.len());
    }

    assert_eq!(buf, [1, 1, 2, 3, 4, 5, 6, 9]);
  }

  #[test]
  fn handles_negatives() {
    let mut buf: [i64; 5] = [3, -1, 0, -10, 5];

    unsafe {
      _zo_arr_sort_i32(buf.as_mut_ptr(), buf.len());
    }

    assert_eq!(buf, [-10, -1, 0, 3, 5]);
  }
}
