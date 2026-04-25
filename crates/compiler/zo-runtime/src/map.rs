//! Runtime hash table backing zo's `HashMap<K, V>`.
//!
//! Open addressing with linear probing, FNV-1a hash,
//! grow at load-factor 0.75 to 2× capacity. Initial
//! capacity 16 slots. Tombstones for deletions; reused
//! on insert when the matching key isn't present
//! earlier in the probe chain.
//!
//! Three key kinds (`KeyKind`):
//!
//! - `Prim` — raw bytes at `key_ptr` are the key.
//!   Covers `int` (any width), `char`, `bool`. The
//!   compiler emits a fixed-size byte buffer at the
//!   call site and passes `&buffer`.
//! - `Str` — `key_ptr` is a zo `str` pointer (header
//!   layout `[len: u64][bytes][null]`). Hash hits the
//!   payload bytes, not the pointer — different heap
//!   copies of the same string hash and compare equal.
//! - `Tuple` — reserved. Not implemented in MVP. The
//!   executor / codegen rejects tuple keys with a
//!   compile-time error today.
//!
//! Storage: each slot owns boxed `Vec<u8>` for key and
//! value. Heap-per-entry is the same trade documented
//! in `channel.rs` — a future pass moves this to
//! inline byte arenas once a benchmark shows it
//! matters.

use crate::str::str_bytes;

const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

const INITIAL_CAPACITY: usize = 16;
const LOAD_FACTOR_NUM: usize = 3;
const LOAD_FACTOR_DEN: usize = 4;

/// Tag identifying how the runtime should hash + compare
/// keys. Passed at `__zo_map_new` and stored on the map
/// for every subsequent op.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyKind {
  Prim = 0,
  Str = 1,
  Tuple = 2,
}

impl KeyKind {
  fn from_u8(v: u8) -> Self {
    match v {
      0 => KeyKind::Prim,
      1 => KeyKind::Str,
      2 => KeyKind::Tuple,
      _ => KeyKind::Prim,
    }
  }
}

#[derive(Clone)]
enum Slot {
  Empty,
  Tombstone,
  Occupied {
    hash: u64,
    key: Vec<u8>,
    val: Vec<u8>,
  },
}

/// Backing struct for `HashMap<K, V>` at the runtime
/// ABI boundary. Pointer-stable: zo holds a `*mut ZoMap`
/// (boxed + leaked) and passes it to every op; the box
/// lives until `__zo_map_free` runs.
pub struct ZoMap {
  slots: Vec<Slot>,
  key_sz: usize,
  val_sz: usize,
  key_kind: KeyKind,
  len: usize,
  tombstones: usize,
}

impl ZoMap {
  fn new(key_kind: KeyKind, key_sz: usize, val_sz: usize, cap: usize) -> Self {
    let cap = cap.max(INITIAL_CAPACITY).next_power_of_two();

    Self {
      slots: vec![Slot::Empty; cap],
      key_sz,
      val_sz,
      key_kind,
      len: 0,
      tombstones: 0,
    }
  }

  fn capacity(&self) -> usize {
    self.slots.len()
  }

  /// FNV-1a over the byte slice.
  fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET_BASIS;

    for b in bytes {
      h ^= *b as u64;
      h = h.wrapping_mul(FNV_PRIME);
    }

    h
  }

  /// Hash a key according to the map's `KeyKind`. For
  /// `Str`, the pointer is dereferenced to its payload
  /// bytes — different heap copies of the same string
  /// produce the same hash.
  ///
  /// # Safety
  ///
  /// `key_ptr` must point at a valid key for this map's
  /// kind: `Prim` requires `key_sz` readable bytes;
  /// `Str` requires a live zo str header.
  unsafe fn hash_key(&self, key_ptr: *const u8) -> u64 {
    match self.key_kind {
      KeyKind::Prim => {
        let bytes = unsafe { std::slice::from_raw_parts(key_ptr, self.key_sz) };

        Self::hash_bytes(bytes)
      }
      KeyKind::Str => {
        let bytes = unsafe { str_bytes(key_ptr) };

        Self::hash_bytes(bytes)
      }
      KeyKind::Tuple => {
        // Reserved. Not reachable from the compiler in
        // MVP — tuple keys are rejected at compile time.
        let bytes = unsafe { std::slice::from_raw_parts(key_ptr, self.key_sz) };

        Self::hash_bytes(bytes)
      }
    }
  }

  /// Read the key at `key_ptr` into an owned `Vec<u8>`
  /// for storage in a slot. For `Str`, the stored bytes
  /// are the payload — never the pointer.
  ///
  /// # Safety
  ///
  /// Same contract as `hash_key`.
  unsafe fn key_to_vec(&self, key_ptr: *const u8) -> Vec<u8> {
    match self.key_kind {
      KeyKind::Prim | KeyKind::Tuple => {
        let bytes = unsafe { std::slice::from_raw_parts(key_ptr, self.key_sz) };

        bytes.to_vec()
      }
      KeyKind::Str => {
        let bytes = unsafe { str_bytes(key_ptr) };

        bytes.to_vec()
      }
    }
  }

  /// Compare the key stored in a slot against an
  /// incoming `key_ptr`. The slot stores the payload
  /// bytes for str keys, so the comparison is byte-wise
  /// regardless of kind.
  ///
  /// # Safety
  ///
  /// Same contract as `hash_key`.
  unsafe fn key_eq(&self, slot_key: &[u8], key_ptr: *const u8) -> bool {
    match self.key_kind {
      KeyKind::Prim | KeyKind::Tuple => {
        let bytes = unsafe { std::slice::from_raw_parts(key_ptr, self.key_sz) };

        slot_key == bytes
      }
      KeyKind::Str => {
        let bytes = unsafe { str_bytes(key_ptr) };

        slot_key == bytes
      }
    }
  }

  /// Locate either the slot holding `key` or the first
  /// free slot in its probe chain. Tombstones are
  /// remembered as candidate insertion points but the
  /// scan continues until an `Empty` slot to ensure
  /// "absent" is decisive.
  ///
  /// Returns `(slot_index, exists)`.
  ///
  /// # Safety
  ///
  /// Same contract as `hash_key`.
  unsafe fn find_slot(&self, key_ptr: *const u8, hash: u64) -> (usize, bool) {
    let mask = self.capacity() - 1;
    let mut idx = (hash as usize) & mask;
    let mut first_tombstone: Option<usize> = None;

    loop {
      match &self.slots[idx] {
        Slot::Empty => {
          return (first_tombstone.unwrap_or(idx), false);
        }
        Slot::Tombstone => {
          if first_tombstone.is_none() {
            first_tombstone = Some(idx);
          }
        }
        Slot::Occupied { hash: h, key, .. } => {
          if *h == hash && unsafe { self.key_eq(key, key_ptr) } {
            return (idx, true);
          }
        }
      }

      idx = (idx + 1) & mask;
    }
  }

  fn should_grow(&self) -> bool {
    (self.len + self.tombstones) * LOAD_FACTOR_DEN
      >= self.capacity() * LOAD_FACTOR_NUM
  }

  /// Double capacity and rehash every occupied slot.
  /// Tombstones are dropped — they don't survive grow.
  fn grow(&mut self) {
    let new_cap = self.capacity() * 2;
    let old = std::mem::replace(&mut self.slots, vec![Slot::Empty; new_cap]);

    self.tombstones = 0;
    self.len = 0;

    let mask = new_cap - 1;

    for slot in old {
      if let Slot::Occupied { hash, key, val } = slot {
        let mut idx = (hash as usize) & mask;

        loop {
          if matches!(self.slots[idx], Slot::Empty) {
            self.slots[idx] = Slot::Occupied { hash, key, val };
            self.len += 1;

            break;
          }

          idx = (idx + 1) & mask;
        }
      }
    }
  }
}

/// Allocate a new map. The returned pointer is
/// `Box::leak`-ed; the matching `__zo_map_free` reclaims
/// it.
///
/// # Safety
///
/// `key_kind` must be a valid `KeyKind` discriminant
/// (0 / 1 / 2). The caller's `key_sz` and `val_sz`
/// must match what every subsequent op assumes.
#[unsafe(export_name = "zo_map_new")]
pub unsafe extern "C-unwind" fn _zo_map_new(
  key_kind: u8,
  key_sz: usize,
  val_sz: usize,
  cap: usize,
) -> *mut ZoMap {
  let map = ZoMap::new(KeyKind::from_u8(key_kind), key_sz, val_sz, cap);

  Box::into_raw(Box::new(map))
}

/// Insert `(key, value)` into the map, overwriting any
/// existing value at the same key. Grows past the load
/// factor before doing the actual insert so the new
/// entry lands in a healthy table.
///
/// # Safety
///
/// `map` must be a live pointer from `__zo_map_new`.
/// `key_ptr` must point at a valid key for the map's
/// kind. `val_ptr` must point at `val_sz` readable
/// bytes.
#[unsafe(export_name = "zo_map_insert")]
pub unsafe extern "C-unwind" fn _zo_map_insert(
  map: *mut ZoMap,
  key_ptr: *const u8,
  val_ptr: *const u8,
) {
  let m = unsafe { &mut *map };

  if m.should_grow() {
    m.grow();
  }

  let hash = unsafe { m.hash_key(key_ptr) };
  let (idx, exists) = unsafe { m.find_slot(key_ptr, hash) };

  let key = unsafe { m.key_to_vec(key_ptr) };
  let val = unsafe { std::slice::from_raw_parts(val_ptr, m.val_sz) }.to_vec();

  let was_tombstone = matches!(m.slots[idx], Slot::Tombstone);

  m.slots[idx] = Slot::Occupied { hash, key, val };

  if !exists {
    m.len += 1;

    if was_tombstone {
      m.tombstones -= 1;
    }
  }
}

/// Look up the value for `key`. On hit, copies the
/// stored value's bytes into `val_out` (the caller's
/// output buffer of size `val_sz`) and returns `true`.
/// On miss, leaves `val_out` untouched and returns
/// `false`.
///
/// # Safety
///
/// `map` must be live; `key_ptr` valid for the map's
/// kind; `val_out` must point at at least `val_sz`
/// writable bytes.
#[unsafe(export_name = "zo_map_get")]
pub unsafe extern "C-unwind" fn _zo_map_get(
  map: *mut ZoMap,
  key_ptr: *const u8,
  val_out: *mut u8,
) -> bool {
  let m = unsafe { &*map };

  let hash = unsafe { m.hash_key(key_ptr) };
  let (idx, exists) = unsafe { m.find_slot(key_ptr, hash) };

  if !exists {
    return false;
  }

  if let Slot::Occupied { val, .. } = &m.slots[idx] {
    unsafe {
      std::ptr::copy_nonoverlapping(val.as_ptr(), val_out, m.val_sz);
    }

    true
  } else {
    false
  }
}

/// `true` iff `key` is in the map.
///
/// # Safety
///
/// As `__zo_map_get`, minus the output buffer.
#[unsafe(export_name = "zo_map_contains")]
pub unsafe extern "C-unwind" fn _zo_map_contains(
  map: *mut ZoMap,
  key_ptr: *const u8,
) -> bool {
  let m = unsafe { &*map };

  let hash = unsafe { m.hash_key(key_ptr) };
  let (_, exists) = unsafe { m.find_slot(key_ptr, hash) };

  exists
}

/// Remove `key`. On hit, copies the removed value into
/// `val_out` and returns `true`. The slot becomes a
/// tombstone — probe chains for other keys still
/// resolve until the next grow rehashes them away.
///
/// # Safety
///
/// As `__zo_map_get`.
#[unsafe(export_name = "zo_map_remove")]
pub unsafe extern "C-unwind" fn _zo_map_remove(
  map: *mut ZoMap,
  key_ptr: *const u8,
  val_out: *mut u8,
) -> bool {
  let m = unsafe { &mut *map };

  let hash = unsafe { m.hash_key(key_ptr) };
  let (idx, exists) = unsafe { m.find_slot(key_ptr, hash) };

  if !exists {
    return false;
  }

  if let Slot::Occupied { val, .. } =
    std::mem::replace(&mut m.slots[idx], Slot::Tombstone)
  {
    unsafe {
      std::ptr::copy_nonoverlapping(val.as_ptr(), val_out, m.val_sz);
    }

    m.len -= 1;
    m.tombstones += 1;

    true
  } else {
    false
  }
}

/// Number of live entries in the map.
///
/// # Safety
///
/// `map` must be live.
#[unsafe(export_name = "zo_map_len")]
pub unsafe extern "C-unwind" fn _zo_map_len(map: *mut ZoMap) -> usize {
  let m = unsafe { &*map };

  m.len
}

/// Free the map. The pointer must NOT be used after
/// this call.
///
/// # Safety
///
/// `map` must be a pointer from `__zo_map_new` that
/// hasn't been freed.
#[unsafe(export_name = "zo_map_free")]
pub unsafe extern "C-unwind" fn _zo_map_free(map: *mut ZoMap) {
  if !map.is_null() {
    unsafe {
      let _ = Box::from_raw(map);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Build a fixed str blob the same shape codegen
  /// emits: `[len: u64 LE][bytes][null]`.
  fn make_str(s: &[u8]) -> Box<[u8]> {
    let mut v = Vec::with_capacity(8 + s.len() + 1);

    v.extend_from_slice(&(s.len() as u64).to_le_bytes());
    v.extend_from_slice(s);
    v.push(0);
    v.into_boxed_slice()
  }

  #[test]
  fn empty_map_has_zero_len() {
    let map =
      unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, INITIAL_CAPACITY) };

    assert_eq!(unsafe { _zo_map_len(map) }, 0);

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn insert_then_get_round_trips_int_keys() {
    let map =
      unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, INITIAL_CAPACITY) };

    for i in 0i32..32 {
      let k = i.to_le_bytes();
      let v = (i * 10).to_le_bytes();

      unsafe {
        _zo_map_insert(map, k.as_ptr(), v.as_ptr());
      }
    }

    assert_eq!(unsafe { _zo_map_len(map) }, 32);

    for i in 0i32..32 {
      let k = i.to_le_bytes();
      let mut out = [0u8; 4];

      let hit = unsafe { _zo_map_get(map, k.as_ptr(), out.as_mut_ptr()) };

      assert!(hit, "missing key {i}");
      assert_eq!(i32::from_le_bytes(out), i * 10);
    }

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn overwrite_same_key_keeps_len_constant() {
    let map =
      unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, INITIAL_CAPACITY) };

    let k = 7i32.to_le_bytes();

    unsafe {
      _zo_map_insert(map, k.as_ptr(), 100i32.to_le_bytes().as_ptr());
    }
    assert_eq!(unsafe { _zo_map_len(map) }, 1);

    unsafe {
      _zo_map_insert(map, k.as_ptr(), 200i32.to_le_bytes().as_ptr());
    }
    assert_eq!(unsafe { _zo_map_len(map) }, 1);

    let mut out = [0u8; 4];

    unsafe {
      _zo_map_get(map, k.as_ptr(), out.as_mut_ptr());
    }
    assert_eq!(i32::from_le_bytes(out), 200);

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn collision_via_low_capacity_still_round_trips() {
    // Force a tight initial capacity (clamped to
    // INITIAL_CAPACITY internally) and exercise the
    // probe chain.
    let map = unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, 1) };

    for i in 0i32..200 {
      let k = i.to_le_bytes();
      let v = (i + 1).to_le_bytes();

      unsafe {
        _zo_map_insert(map, k.as_ptr(), v.as_ptr());
      }
    }

    for i in 0i32..200 {
      let k = i.to_le_bytes();
      let mut out = [0u8; 4];

      assert!(unsafe { _zo_map_get(map, k.as_ptr(), out.as_mut_ptr()) });
      assert_eq!(i32::from_le_bytes(out), i + 1);
    }

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn remove_then_reinsert_reuses_tombstone() {
    let map =
      unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, INITIAL_CAPACITY) };

    let k = 42i32.to_le_bytes();
    let v = 1000i32.to_le_bytes();

    unsafe {
      _zo_map_insert(map, k.as_ptr(), v.as_ptr());
    }

    let mut removed = [0u8; 4];

    let did = unsafe { _zo_map_remove(map, k.as_ptr(), removed.as_mut_ptr()) };

    assert!(did);
    assert_eq!(i32::from_le_bytes(removed), 1000);
    assert_eq!(unsafe { _zo_map_len(map) }, 0);

    let v2 = 2000i32.to_le_bytes();

    unsafe {
      _zo_map_insert(map, k.as_ptr(), v2.as_ptr());
    }

    let mut out = [0u8; 4];

    unsafe {
      _zo_map_get(map, k.as_ptr(), out.as_mut_ptr());
    }
    assert_eq!(i32::from_le_bytes(out), 2000);
    assert_eq!(unsafe { _zo_map_len(map) }, 1);

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn contains_distinguishes_present_and_absent() {
    let map =
      unsafe { _zo_map_new(KeyKind::Prim as u8, 4, 4, INITIAL_CAPACITY) };

    let k = 5i32.to_le_bytes();

    assert!(!unsafe { _zo_map_contains(map, k.as_ptr()) });

    unsafe {
      _zo_map_insert(map, k.as_ptr(), 99i32.to_le_bytes().as_ptr());
    }

    assert!(unsafe { _zo_map_contains(map, k.as_ptr()) });

    let other = 6i32.to_le_bytes();

    assert!(!unsafe { _zo_map_contains(map, other.as_ptr()) });

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn str_keys_hash_by_content_not_pointer() {
    let map =
      unsafe { _zo_map_new(KeyKind::Str as u8, 8, 4, INITIAL_CAPACITY) };

    let k1 = make_str(b"hello");
    let k2 = make_str(b"hello");

    let p1 = k1.as_ptr();
    let p2 = k2.as_ptr();

    // Different heap copies of the same bytes — pointer
    // hashing would put them in different slots.
    assert_ne!(p1, p2);

    unsafe {
      _zo_map_insert(map, p1, 1i32.to_le_bytes().as_ptr());
    }

    let mut out = [0u8; 4];

    let hit = unsafe { _zo_map_get(map, p2, out.as_mut_ptr()) };

    assert!(hit, "content-equal str keys must hash to the same bucket");
    assert_eq!(i32::from_le_bytes(out), 1);

    unsafe {
      _zo_map_free(map);
    }
  }

  #[test]
  fn distinct_str_keys_dont_collide_under_grow() {
    let map = unsafe { _zo_map_new(KeyKind::Str as u8, 8, 4, 1) };

    let bytes = [
      &b"alpha"[..],
      &b"beta"[..],
      &b"gamma"[..],
      &b"delta"[..],
      &b"epsilon"[..],
      &b"zeta"[..],
      &b"eta"[..],
      &b"theta"[..],
      &b"iota"[..],
      &b"kappa"[..],
      &b"lambda"[..],
      &b"mu"[..],
      &b"nu"[..],
      &b"xi"[..],
      &b"omicron"[..],
      &b"pi"[..],
      &b"rho"[..],
      &b"sigma"[..],
      &b"tau"[..],
      &b"upsilon"[..],
    ];

    let blobs: Vec<Box<[u8]>> = bytes.iter().map(|b| make_str(b)).collect();

    for (i, blob) in blobs.iter().enumerate() {
      let v = (i as i32).to_le_bytes();

      unsafe {
        _zo_map_insert(map, blob.as_ptr(), v.as_ptr());
      }
    }

    assert_eq!(unsafe { _zo_map_len(map) }, blobs.len());

    for (i, blob) in blobs.iter().enumerate() {
      let mut out = [0u8; 4];

      assert!(unsafe { _zo_map_get(map, blob.as_ptr(), out.as_mut_ptr()) });
      assert_eq!(i32::from_le_bytes(out), i as i32);
    }

    unsafe {
      _zo_map_free(map);
    }
  }
}
