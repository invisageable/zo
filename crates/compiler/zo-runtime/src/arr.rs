//! Runtime helpers for `[]T` arrays.
//!
//! `_zo_arr_sort_i32` sorts a `[]int` (default int = i32)
//! in place via Rust's stable slice sort. Mirrors the
//! pattern used by `zo_map_*` / `zo_vec_*` — the compiler
//! emits a direct `BL _zo_arr_sort_i32` from
//! `arr_int::sort`'s codegen handler with `X0` pointing
//! at the array's data and `X1` carrying the element
//! count.

/// Header bytes prefixing every runtime `[]T`:
/// `[len: u64][cap: u64]`. Element data starts at this
/// offset.
pub(crate) const RUNTIME_ARRAY_HEADER_SIZE: usize = 16;

/// Width of one slot in a runtime `[]T`. Every element
/// occupies 8 bytes regardless of declared type — pointers
/// and 64-bit primitives store inline; narrower primitives
/// zero-extend.
pub(crate) const RUNTIME_SLOT_SIZE: usize = 8;

/// Allocate `n` uninitialized bytes on the heap, hand them
/// to `fill` for a single write pass, then leak the buffer
/// and return its pointer. Mirrors `str::alloc_str_with` —
/// `MaybeUninit::new_uninit_slice` skips the redundant zero
/// pass `vec![0u8; n]` would impose before the overwrite.
fn alloc_leaked_bytes(n: usize, fill: impl FnOnce(&mut [u8])) -> *const u8 {
  let mut buf = Box::<[u8]>::new_uninit_slice(n);
  let ptr = buf.as_mut_ptr() as *mut u8;

  // SAFETY: `ptr` is valid for `n` writes (the box owns
  // them); `fill` writes every byte before its `&mut`
  // borrow ends, so `assume_init` sees a fully-initialized
  // buffer.
  unsafe {
    fill(std::slice::from_raw_parts_mut(ptr, n));
  }

  Box::leak(unsafe { buf.assume_init() }).as_ptr()
}

/// Allocate a runtime `[]ptr` containing the given
/// pointer-shaped elements. Layout matches the codegen's
/// `[len:u64][cap:u64][slot0:u64]...[slotN:u64]`. Used by
/// FFI helpers that return `[]str` / `[]ptr` to user code.
///
/// `cap == len` because these arrays are immutable from
/// the user's view; `push` would hit the codegen's heap
/// allocator, not these helpers.
pub(crate) fn alloc_ptr_array(elements: &[*const u8]) -> *const u8 {
  let n = elements.len();
  let total = RUNTIME_ARRAY_HEADER_SIZE + n * RUNTIME_SLOT_SIZE;

  alloc_leaked_bytes(total, |arr| {
    let len_le = (n as u64).to_le_bytes();

    arr[0..8].copy_from_slice(&len_le);
    arr[8..16].copy_from_slice(&len_le);

    for (i, ptr) in elements.iter().enumerate() {
      let off = RUNTIME_ARRAY_HEADER_SIZE + i * RUNTIME_SLOT_SIZE;

      arr[off..off + RUNTIME_SLOT_SIZE]
        .copy_from_slice(&(*ptr as usize as u64).to_le_bytes());
    }
  })
}

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

/// Heap-clone `n` bytes starting at `src`, returning a fresh
/// leaked pointer. `Insn::ArrayPush` codegen calls this to
/// snapshot a struct value at push time so the struct's
/// frame slot can be reused for the next iteration without
/// aliasing previously-pushed array elements.
///
/// # Safety
///
/// `src` must point at `n` readable bytes.
#[unsafe(export_name = "zo_box_alloc")]
pub unsafe extern "C-unwind" fn _zo_box_alloc(
  src: *const u8,
  n: usize,
) -> *const u8 {
  alloc_leaked_bytes(n, |dst| unsafe {
    std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), n);
  })
}

/// `any <Abstract>` fat-pointer constructor. Allocates
/// exactly 16 bytes of header — the data pointer the
/// caller hands in is stored verbatim at slot 0, the
/// vtable pointer goes to slot 1. The original concrete
/// value stays where it is (heap, stack frame, wherever
/// the caller's binding holds it); the fat pointer
/// merely aliases it for vtable dispatch.
///
/// ```text
/// [fat + 0 .. 8]   data_ptr   = `data` argument verbatim
/// [fat + 8 .. 16]  vtable_ptr
/// ```
///
/// This level of indirection is the minimum that lets
/// `<dyn>.method()` call into the underlying impl: the
/// `self` register passed to the dispatched method is
/// the data pointer, so the method reads field offsets
/// directly off the original value's layout. The
/// caller is responsible for keeping `data` alive
/// across the lifetime of the fat pointer; `_zo_dyn_free`
/// releases only the 16-byte header.
///
/// # Safety
///
/// `data` must remain valid for as long as the returned
/// fat pointer is in use; `vtable_ptr` must point at a
/// vtable laid out per the codegen's `emit_vtables`
/// contract (`[size_of_data, method_0, ..., method_N]`).
#[unsafe(export_name = "zo_dyn_box")]
pub unsafe extern "C-unwind" fn _zo_dyn_box(
  data: *const u8,
  vtable_ptr: *const u8,
) -> *const u8 {
  alloc_leaked_bytes(16, |buf| {
    let data_addr = data as usize as u64;
    let vt_addr = vtable_ptr as usize as u64;

    buf[0..8].copy_from_slice(&data_addr.to_le_bytes());
    buf[8..16].copy_from_slice(&vt_addr.to_le_bytes());
  })
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
