//! zo-provider-json — runtime backing the user-facing
//! `compiler-lib/core/json.zo`. Wraps `serde_json` behind
//! a small C ABI surface so zo programs can parse, read,
//! and serialize JSON without an in-language sum type or
//! recursive tree representation.
//!
//! Handle protocol (Decision 8 of `PLAN_PROVIDER_JSON.md`):
//! every user-visible handle is a 1-based `i64` index into
//! `HANDLES`. Each handle carries `(root, path)` so sub-
//! value access appends one `Step` per `.get()` /
//! `.get_at()` call instead of cloning a `serde_json::Value`
//! subtree. `0` is the failure sentinel — parse error,
//! freed slot, missing key, wrong kind.
//!
//! Roots are refcounted because sibling handles share a
//! parsed tree (`obj.get("a")` and `obj.get("b")` both
//! point at `obj`'s root). The root drops when the last
//! handle referencing it is freed.

use zo_c_abi::{CBytes, stage_cbytes};

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::LazyLock;
use std::sync::Mutex;

use serde_json::Value;
use smallvec::SmallVec;

/// Inline path-step capacity. JSON traversals deeper than
/// four hops fall back to the heap; the typical case
/// (`doc.get("field")` / `doc.get("k").get_at(i)`) stays
/// stack-allocated and skips the per-traversal `Vec`
/// allocation.
type StepPath = SmallVec<[Step; 4]>;

// Per-thread scratch for the string-returning FFIs
// (`__zo_json_to_str` / `__zo_json_as_str` /
// `__zo_json_key_at`). Each call overwrites the previous
// — safe because the bytes are immediately copied via
// `CBytes::to_str()` on the zo side.
//
// `KEYS` is a separate per-thread buffer for object key
// iteration. `keys_len` fills it from the object; each
// `key_at(handle, index)` refills before reading so two
// iterators on different handles can interleave without
// trampling. Cost is O(n) per index, accepted for the
// expected object sizes; a generation-tagged cache lives
// behind the same buffer if profiling demands it.
thread_local! {
  static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
  static KEYS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
  // `(handle, generation)` of the value whose keys are
  // currently in `KEYS`. `None` when no cache is staged.
  // `key_at` skips the rebuild when the owner still
  // matches — turning the canonical `for i in
  // 0..obj.keys_len() { obj.key_at(i) }` loop from O(n²)
  // back into O(n). Bumped via `RootSlot.generation` on
  // every `push` / `set`, so a mutation in the same
  // iteration invalidates the cache automatically.
  static KEYS_OWNER: Cell<Option<(ZoHandle, u32)>> =
    const { Cell::new(None) };
  // Most recent parse error message on this thread. `Some`
  // when the last `parse_str` failed; cleared to `None`
  // when a parse succeeds so callers can detect success /
  // failure from the value alone.
  static LAST_ERROR: RefCell<Option<String>> =
    const { RefCell::new(None) };
}

/// Owned parse + refcount. Handles point in; the count
/// tracks how many are alive so we can drop the value when
/// it falls to zero.
#[derive(Debug)]
struct RootSlot {
  value: Value,
  refcount: u32,
  /// Bumped on every mutation (`push` / `set`). Pairs
  /// with the per-thread `KEYS_OWNER` to skip rebuilding
  /// the object-key cache when the underlying object
  /// hasn't changed since the last `key_at` call.
  generation: u32,
}

/// Traversal step within a root. `Json::get(key)` appends
/// `Key`; `Json::get_at(idx)` appends `Idx`. Walking the
/// path on every scalar access avoids cloning subtrees.
#[derive(Clone, Debug)]
enum Step {
  Key(String),
  Idx(u32),
}

/// User-facing handle payload. `root` indexes into `ROOTS`;
/// `path` is empty for the root handle returned by parse
/// and grows by one `Step` per traversal call.
#[derive(Clone, Debug)]
struct Handle {
  root: u32,
  path: StepPath,
}

/// Roots + handles share a single lock so every FFI
/// entry point acquires exactly one mutex per call. The
/// type-level coupling also makes lock-order bugs
/// unrepresentable: there's only one lock, no order to
/// get wrong.
#[derive(Default)]
struct Registry {
  roots: Vec<Option<RootSlot>>,
  handles: Vec<Option<Handle>>,
}

static REGISTRY: LazyLock<Mutex<Registry>> =
  LazyLock::new(|| Mutex::new(Registry::default()));

/// zo-side `int` is i64, matching the AAPCS GP register.
type ZoHandle = i64;

/// JSON kind discriminator returned to zo. Decision 3 of
/// the plan — keep in lockstep with `core/json.zo` when
/// the binding promotes these to named constants.
const KIND_NULL: ZoHandle = 0;
const KIND_BOOL: ZoHandle = 1;
const KIND_NUMBER: ZoHandle = 2;
const KIND_STRING: ZoHandle = 3;
const KIND_ARRAY: ZoHandle = 4;
const KIND_OBJECT: ZoHandle = 5;

/// Allocate a fresh handle row pointing at `root` with the
/// supplied `path`. Increments the root's refcount so the
/// shared parse tree survives until every handle drops.
/// Returns the slot index + 1 (zo's 1-based handle).
fn alloc_handle(
  roots: &mut [Option<RootSlot>],
  handles: &mut Vec<Option<Handle>>,
  root: u32,
  path: StepPath,
) -> ZoHandle {
  if let Some(Some(slot)) = roots.get_mut(root as usize) {
    slot.refcount += 1;
  } else {
    return 0;
  }

  handles.push(Some(Handle { root, path }));
  handles.len() as ZoHandle
}

/// Read a NUL-terminated UTF-8 C string from `ptr`. zo
/// passes these via `c_str(...)` — bytes after the 8-byte
/// length prefix, guaranteed NUL-terminated by the
/// tokenizer's rodata layout.
unsafe fn read_c_str<'a>(ptr: *const c_char) -> Option<&'a str> {
  if ptr.is_null() {
    return None;
  }

  unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Walk `handle`'s path from its root and return a
/// reference into the parsed tree. `None` when the slot is
/// freed or the path no longer resolves (key removed,
/// index out of bounds, kind mismatch at some hop).
#[inline]
fn resolve<'a>(
  handle: ZoHandle,
  roots: &'a [Option<RootSlot>],
  handles: &[Option<Handle>],
) -> Option<&'a Value> {
  let h = handles.get((handle - 1) as usize)?.as_ref()?;
  let slot = roots.get(h.root as usize)?.as_ref()?;
  let mut cur = &slot.value;

  for step in &h.path {
    cur = match (cur, step) {
      (Value::Object(obj), Step::Key(k)) => obj.get(k)?,
      (Value::Array(arr), Step::Idx(i)) => arr.get(*i as usize)?,
      _ => return None,
    };
  }

  Some(cur)
}

/// Mutable variant of `resolve`. Used by the builder
/// mutators (`__zo_json_push` / `__zo_json_set`) which need
/// to modify the underlying `Value` at the handle's path.
#[inline]
fn resolve_mut<'a>(
  handle: ZoHandle,
  roots: &'a mut [Option<RootSlot>],
  handles: &[Option<Handle>],
) -> Option<&'a mut Value> {
  let h = handles.get((handle - 1) as usize)?.as_ref()?;
  let slot = roots.get_mut(h.root as usize)?.as_mut()?;
  let mut cur = &mut slot.value;

  for step in &h.path {
    cur = match (cur, step) {
      (Value::Object(obj), Step::Key(k)) => obj.get_mut(k)?,
      (Value::Array(arr), Step::Idx(i)) => arr.get_mut(*i as usize)?,
      _ => return None,
    };
  }

  Some(cur)
}

/// Push a freshly-built `Value` into a new root and return
/// the handle. Shared by every primitive / container
/// builder (`null`, `from_bool`, `array`, `object`, …) so
/// the refcount + handle bookkeeping happens in one place.
fn push_new_root(value: Value) -> ZoHandle {
  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  roots.push(Some(RootSlot {
    value,
    refcount: 0,
    generation: 0,
  }));

  let root_idx = (roots.len() - 1) as u32;

  alloc_handle(roots, handles, root_idx, StepPath::new())
}

/// `__zo_json_parse_str(ptr: int) -> int`. Parse a NUL-
/// terminated C string into a fresh root + handle. Returns
/// the positive handle on success, `0` on parse error
/// (invalid JSON, bad UTF-8, OOM).
///
/// # Safety
///
/// `ptr` must be a valid NUL-terminated UTF-8 C string or
/// null. Caller passes ownership of the buffer for the
/// duration of the call; the parsed `Value` owns its own
/// memory afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __zo_json_parse_str(ptr: *const c_char) -> ZoHandle {
  let Some(s) = (unsafe { read_c_str(ptr) }) else {
    LAST_ERROR.with(|cell| {
      *cell.borrow_mut() = Some("input pointer is null or not utf-8".into());
    });
    return 0;
  };

  let value = match serde_json::from_str::<Value>(s) {
    Ok(v) => {
      LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
      v
    }
    Err(e) => {
      LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(e.to_string()));
      return 0;
    }
  };

  push_new_root(value)
}

/// `__zo_json_free(handle: int)`. Idempotent — freeing a
/// handle that's already gone (or was never valid) is a
/// no-op. Decrements the root's refcount and drops the
/// underlying `Value` when the last reference goes.
///
/// Locks ROOTS before HANDLES to match every other FFI
/// entry point — the inverse order is a latent ABBA the
/// moment a second thread enters the registry.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_free(handle: ZoHandle) {
  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  let Some(slot) = handles.get_mut((handle - 1) as usize) else {
    return;
  };

  let Some(h) = slot.take() else {
    return;
  };

  let Some(Some(rs)) = roots.get_mut(h.root as usize) else {
    return;
  };

  rs.refcount = rs.refcount.saturating_sub(1);

  if rs.refcount == 0 {
    roots[h.root as usize] = None;
  }
}

/// `__zo_json_kind(handle: int) -> int`. Returns the kind
/// discriminator. `KIND_NULL` for an invalid / freed slot;
/// callers disambiguate from a real `null` by checking the
/// handle against `0` before any other call.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_kind(handle: ZoHandle) -> ZoHandle {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  let Some(value) = resolve(handle, roots, handles) else {
    return KIND_NULL;
  };

  match value {
    Value::Null => KIND_NULL,
    Value::Bool(_) => KIND_BOOL,
    Value::Number(_) => KIND_NUMBER,
    Value::String(_) => KIND_STRING,
    Value::Array(_) => KIND_ARRAY,
    Value::Object(_) => KIND_OBJECT,
  }
}

/// `__zo_json_as_bool(handle: int) -> int`. Returns `1` /
/// `0` when the underlying value is a JSON boolean, `0`
/// otherwise (matching zo's wrapper semantics: a wrong-kind
/// access yields the zero-equivalent of the asked type).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_as_bool(handle: ZoHandle) -> ZoHandle {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  match resolve(handle, roots, handles) {
    Some(Value::Bool(b)) => *b as ZoHandle,
    _ => 0,
  }
}

/// `__zo_json_as_i64(handle: int) -> int`. Reads the
/// underlying number as `i64`. Returns `0` for non-number
/// kinds AND for numbers that don't fit (NaN / `Infinity`
/// can't appear — serde rejects them on parse — but a
/// `u64` past `i64::MAX` does). Callers who need to
/// disambiguate "real zero" from "wrong kind / overflow"
/// should check `kind()` and `is_int()` first.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_as_i64(handle: ZoHandle) -> ZoHandle {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  match resolve(handle, roots, handles) {
    Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
    _ => 0,
  }
}

/// `__zo_json_as_f64(handle: int) -> float`. Reads the
/// underlying number as `f64`. Works for integer-valued
/// numbers too (serde converts), so this is the safe
/// fallback for unknown numeric schemas. `0.0` for non-
/// number kinds.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_as_f64(handle: ZoHandle) -> f64 {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  match resolve(handle, roots, handles) {
    Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
    _ => 0.0,
  }
}

/// `__zo_json_is_int(handle: int) -> int`. Returns `1`
/// when the underlying number's serde representation is
/// integral (`i64` or `u64` arm), `0` for floats and for
/// non-number kinds. Lets `as_int` callers detect a
/// truncating read before it happens.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_is_int(handle: ZoHandle) -> ZoHandle {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  match resolve(handle, roots, handles) {
    Some(Value::Number(n)) => (n.is_i64() || n.is_u64()) as ZoHandle,
    _ => 0,
  }
}

/// `__zo_json_len(handle: int) -> int`. Array length /
/// object key count. `0` for scalar kinds, invalid handles,
/// and (legitimately) empty containers. Pair with `kind()`
/// when the schema admits both empty-object and scalar.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_len(handle: ZoHandle) -> ZoHandle {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  match resolve(handle, roots, handles) {
    Some(Value::Array(arr)) => arr.len() as ZoHandle,
    Some(Value::Object(obj)) => obj.len() as ZoHandle,
    _ => 0,
  }
}

/// `__zo_json_get(handle: int, key: int) -> int`. Object
/// field access. Returns a new handle whose path is the
/// parent's `+ Step::Key(key)`. `0` for non-object kinds,
/// missing keys, or invalid handles.
///
/// # Safety
///
/// `key` must be a valid NUL-terminated UTF-8 C string or
/// null. Caller passes ownership for the duration of the
/// call; the resolved `String` is cloned into the new
/// handle's path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __zo_json_get(
  handle: ZoHandle,
  key: *const c_char,
) -> ZoHandle {
  let Some(key) = (unsafe { read_c_str(key) }) else {
    return 0;
  };

  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  let exists = matches!(
    resolve(handle, roots, handles),
    Some(Value::Object(obj)) if obj.contains_key(key),
  );

  if !exists {
    return 0;
  }

  let parent = handles[(handle - 1) as usize].as_ref().unwrap();
  let root = parent.root;
  let mut new_path = parent.path.clone();

  new_path.push(Step::Key(key.to_owned()));
  alloc_handle(roots, handles, root, new_path)
}

/// `__zo_json_get_at(handle: int, idx: int) -> int`. Array
/// index access. Returns a new handle whose path is the
/// parent's `+ Step::Idx(idx)`. `0` for non-array kinds,
/// out-of-range indices, or invalid handles.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_get_at(
  handle: ZoHandle,
  idx: ZoHandle,
) -> ZoHandle {
  if idx < 0 {
    return 0;
  }

  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  let in_range = matches!(
    resolve(handle, roots, handles),
    Some(Value::Array(arr)) if (idx as usize) < arr.len(),
  );

  if !in_range {
    return 0;
  }

  let parent = handles[(handle - 1) as usize].as_ref().unwrap();
  let root = parent.root;
  let mut new_path = parent.path.clone();

  new_path.push(Step::Idx(idx as u32));
  alloc_handle(roots, handles, root, new_path)
}

/// Serialize the value at `handle` as compact JSON.
///
/// @note — empty bytes on invalid handle or serialization
/// failure. `as_str` returns the unquoted string content;
/// `to_str` re-encodes through serde so a parsed `"hi"`
/// round-trips as `"\"hi\""`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_to_str(handle: ZoHandle) -> CBytes {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  let Some(value) = resolve(handle, roots, handles) else {
    return CBytes::empty();
  };

  let Ok(s) = serde_json::to_string(value) else {
    return CBytes::empty();
  };

  stage_cbytes(&OUT, s.as_bytes())
}

/// Read a JSON string's content (unquoted).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_as_str(handle: ZoHandle) -> CBytes {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  let bytes: &[u8] = match resolve(handle, roots, handles) {
    Some(Value::String(s)) => s.as_bytes(),
    _ => b"",
  };

  stage_cbytes(&OUT, bytes)
}

/// Populate the per-thread KEYS cache from `handle`'s
/// object value. Returns the key count (`0` for non-
/// objects / invalid handles).
///
/// Skips the rebuild when the cached `(handle,
/// generation)` still matches — the canonical iteration
/// pattern `for i in 0..obj.keys_len() { obj.key_at(i) }`
/// stays O(n) instead of O(n²). Mutations on the root
/// bump `RootSlot.generation` which invalidates the
/// cache automatically.
fn fill_keys_cache(handle: ZoHandle) -> usize {
  let reg = REGISTRY.lock().unwrap();
  let roots = &reg.roots;
  let handles = &reg.handles;

  let root_generation = handles
    .get((handle - 1) as usize)
    .and_then(|h| h.as_ref())
    .and_then(|h| roots.get(h.root as usize))
    .and_then(|r| r.as_ref())
    .map(|slot| slot.generation);

  if let Some(generation) = root_generation
    && KEYS_OWNER.get() == Some((handle, generation))
  {
    return KEYS.with(|cell| cell.borrow().len());
  }

  let new_keys: Vec<String> = match resolve(handle, roots, handles) {
    Some(Value::Object(obj)) => obj.keys().cloned().collect(),
    _ => Vec::new(),
  };

  let n = new_keys.len();

  KEYS.with(|cell| *cell.borrow_mut() = new_keys);
  KEYS_OWNER.set(root_generation.map(|g| (handle, g)));
  n
}

/// `__zo_json_keys_len(handle: int) -> int`. Materialize
/// the object's keys into the per-thread cache and return
/// the count. `0` for non-object kinds and invalid
/// handles. Decision 10: eager materialization over an
/// iterator handle — small allocation cost, no second
/// registry, lifetime stays trivial.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_keys_len(handle: ZoHandle) -> ZoHandle {
  fill_keys_cache(handle) as ZoHandle
}

/// Return the i-th cached key from `handle`'s object.
///
/// @note — re-populates the per-thread cache from `handle`
/// per call so two iterators on different handles can
/// interleave without trampling. Empty bytes for non-
/// objects, out-of-range, or invalid handles.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_key_at(
  handle: ZoHandle,
  index: ZoHandle,
) -> CBytes {
  if index < 0 {
    return CBytes::empty();
  }

  fill_keys_cache(handle);

  KEYS.with(|cell| {
    let keys = cell.borrow();
    let bytes = keys
      .get(index as usize)
      .map(|k| k.as_bytes())
      .unwrap_or(b"");

    stage_cbytes(&OUT, bytes)
  })
}

/// Most recent parse error message on this thread.
///
/// @note — empty when the last `parse_str` succeeded or no
/// parse has run yet. Pair with `Json::parse(...)` returning
/// handle `0` to surface a human-readable reason.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_last_error() -> CBytes {
  LAST_ERROR.with(|cell| {
    let cell = cell.borrow();
    let bytes = cell.as_ref().map(|s| s.as_bytes()).unwrap_or(b"");

    stage_cbytes(&OUT, bytes)
  })
}

// ============================================================
// Builders. Each produces a fresh root + handle. zo-side
// `Json::null()` / `Json::from_*` / `Json::array()` /
// `Json::object()` wrap these so users construct JSON
// values from native zo data without round-tripping through
// `parse(stringified)`.
// ============================================================

/// `__zo_json_null() -> int`. Fresh `null` root.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_null() -> ZoHandle {
  push_new_root(Value::Null)
}

/// zo's `bool` is marshaled as `int`; non-zero is `true`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_from_bool(value: ZoHandle) -> ZoHandle {
  push_new_root(Value::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_from_int(value: ZoHandle) -> ZoHandle {
  push_new_root(Value::Number(value.into()))
}

/// Returns `0` for NaN / Infinity — serde refuses to
/// construct a `Number` from non-finite floats, matching
/// JSON's RFC 8259 (no non-finite literals).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_from_f64(value: f64) -> ZoHandle {
  match serde_json::Number::from_f64(value) {
    Some(num) => push_new_root(Value::Number(num)),
    None => {
      LAST_ERROR.with(|cell| {
        *cell.borrow_mut() =
          Some("from_f64: NaN / Infinity not representable in JSON".into());
      });
      0
    }
  }
}

/// `__zo_json_from_str(ptr: int) -> int`. Fresh string
/// root. Returns `0` on null / non-UTF-8 input.
///
/// # Safety
///
/// `ptr` must be a valid NUL-terminated UTF-8 C string or
/// null. The string is copied into the new `Value`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __zo_json_from_str(ptr: *const c_char) -> ZoHandle {
  match unsafe { read_c_str(ptr) } {
    Some(s) => push_new_root(Value::String(s.to_owned())),
    None => 0,
  }
}

/// `__zo_json_array() -> int`. Fresh empty-array root.
/// Grow it via `__zo_json_push`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_array() -> ZoHandle {
  push_new_root(Value::Array(Vec::new()))
}

/// `__zo_json_object() -> int`. Fresh empty-object root.
/// Grow it via `__zo_json_set`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_object() -> ZoHandle {
  push_new_root(Value::Object(serde_json::Map::new()))
}

/// `__zo_json_push(self: int, val: int) -> int`. Append a
/// **clone** of `val`'s value to `self`'s array. Returns
/// `1` on success, `0` on failure (wrong kind, invalid
/// handle). Cloning is necessary because `val` retains
/// ownership of its own root — freeing `self` later must
/// not free `val`'s slot and vice versa.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_json_push(
  self_handle: ZoHandle,
  value_handle: ZoHandle,
) -> ZoHandle {
  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  let Some(value_clone) = resolve(value_handle, roots, handles).cloned() else {
    return 0;
  };

  let target_root = handles
    .get((self_handle - 1) as usize)
    .and_then(|h| h.as_ref())
    .map(|h| h.root as usize);

  let Some(target) = resolve_mut(self_handle, roots, handles) else {
    return 0;
  };

  let success = matches!(target, Value::Array(_));
  if let Value::Array(arr) = target {
    arr.push(value_clone);
  }

  if success
    && let Some(idx) = target_root
    && let Some(Some(slot)) = roots.get_mut(idx)
  {
    slot.generation = slot.generation.wrapping_add(1);
  }

  if success { 1 } else { 0 }
}

/// `__zo_json_set(self: int, key: int, val: int) -> int`.
/// Insert (or overwrite) `key → clone(val)` in `self`'s
/// object. Returns `1` on success, `0` on failure (wrong
/// kind, invalid handle, null / non-UTF-8 key).
///
/// # Safety
///
/// `key` must be a valid NUL-terminated UTF-8 C string or
/// null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __zo_json_set(
  self_handle: ZoHandle,
  key: *const c_char,
  value_handle: ZoHandle,
) -> ZoHandle {
  let Some(k) = (unsafe { read_c_str(key) }) else {
    return 0;
  };

  let key_owned = k.to_owned();
  let mut reg = REGISTRY.lock().unwrap();
  let Registry { roots, handles } = &mut *reg;

  let Some(value_clone) = resolve(value_handle, roots, handles).cloned() else {
    return 0;
  };

  let target_root = handles
    .get((self_handle - 1) as usize)
    .and_then(|h| h.as_ref())
    .map(|h| h.root as usize);

  let Some(target) = resolve_mut(self_handle, roots, handles) else {
    return 0;
  };

  let success = matches!(target, Value::Object(_));
  if let Value::Object(obj) = target {
    obj.insert(key_owned, value_clone);
  }

  if success
    && let Some(idx) = target_root
    && let Some(Some(slot)) = roots.get_mut(idx)
  {
    slot.generation = slot.generation.wrapping_add(1);
  }

  if success { 1 } else { 0 }
}

/// `__zo_json_write(self: int, path: int) -> int`.
/// Serialize the value as compact JSON and write it to
/// `path`. Returns `0` on success, non-zero on failure
/// (invalid handle, serialization error, IO error). The
/// failure reason is staged in `LAST_ERROR` — read it via
/// `Json::last_error()`.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated UTF-8 C string or
/// null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __zo_json_write(
  handle: ZoHandle,
  path: *const c_char,
) -> ZoHandle {
  let Some(p) = (unsafe { read_c_str(path) }) else {
    LAST_ERROR.with(|cell| {
      *cell.borrow_mut() = Some("write: path is null or not utf-8".into());
    });
    return 1;
  };

  let s = {
    let reg = REGISTRY.lock().unwrap();
    let roots = &reg.roots;
    let handles = &reg.handles;

    let Some(value) = resolve(handle, roots, handles) else {
      LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some("write: invalid handle".into());
      });
      return 1;
    };

    match serde_json::to_string(value) {
      Ok(s) => s,
      Err(e) => {
        LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(e.to_string()));
        return 1;
      }
    }
  };

  match std::fs::write(p, s) {
    Ok(()) => {
      LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
      0
    }
    Err(e) => {
      LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(e.to_string()));
      1
    }
  }
}
