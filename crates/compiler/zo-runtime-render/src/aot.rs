//! Platform-agnostic ABI + reactive plumbing for AOT-compiled
//! zo programs.
//!
//! Every UI backend (eframe on desktop, UIKit on iOS, Android
//! views) shares this layer: the `ZoRuntimeContext` the compiled
//! exe passes in, the reactive `_zo_state_*` buffer the closures
//! mutate, the postcard template decode, the `TextBinding`
//! refresh, and the `EventRegistry` builder. Only the final
//! "draw the commands + run the event loop" step is
//! platform-specific, so each backend's `zo_run_native` decodes
//! and builds the registry through here, then drives its own UI.

use crate::reactive::{
  BindingGraph, BindingRef, DirtyCommands, DirtySet, ListEdit, reconcile_list,
  refresh_dirty,
};
use crate::render::{EventHandler, EventRegistry};

use zo_ui_protocol::codec::{self, CodecError};
use zo_ui_protocol::{LIST_ITEM_SENTINEL, UiCommand};

use std::collections::HashSet;
use std::slice;
use std::sync::{Arc, Mutex, OnceLock};

/// The global reactive state, one `Vec<i64>` per program.
static STATE: OnceLock<Mutex<Vec<i64>>> = OnceLock::new();

/// The string-typed reactive state.
static STR_STATE: OnceLock<Mutex<Vec<Vec<u8>>>> = OnceLock::new();

/// The array-typed reactive state — one `Vec<String>` per slot,
/// backing `mut []str` reactive arrays (the list-binding
/// `items_var`). A compiled handler's `todos.push(x)` lowers to
/// `zo_state_arr_push`, which the list refresh reads to re-render
/// the `<ul>`. Slots share the one id space with `STATE` /
/// `STR_STATE`; a given slot is only ever one kind.
static ARR_STATE: OnceLock<Mutex<Vec<Vec<String>>>> = OnceLock::new();

/// Slots written since the last drain. A state write marks its
/// slot here; `drain_dirty` empties it after each event so the
/// runtime refreshes only the bindings that actually changed.
static DIRTY: OnceLock<Mutex<DirtySet>> = OnceLock::new();

fn state() -> &'static Mutex<Vec<i64>> {
  STATE.get_or_init(|| Mutex::new(Vec::new()))
}

fn str_state() -> &'static Mutex<Vec<Vec<u8>>> {
  STR_STATE.get_or_init(|| Mutex::new(Vec::new()))
}

fn arr_state() -> &'static Mutex<Vec<Vec<String>>> {
  ARR_STATE.get_or_init(|| Mutex::new(Vec::new()))
}

fn dirty() -> &'static Mutex<DirtySet> {
  DIRTY.get_or_init(|| Mutex::new(DirtySet::default()))
}

/// Append every slot written since the last call to `into`
/// (ascending), then clear the dirty set. `into` is the
/// caller's reused scratch buffer — no allocation on the event
/// path.
pub fn drain_dirty(into: &mut Vec<u32>) {
  dirty().lock().unwrap().drain_into(into);
}

/// Encode `bytes` into `[len: u64][bytes][null]` layout — mirrors
/// `Insn::ConstString` so the result is a drop-in replacement for a static
/// string literal.
fn encode_length_prefixed(bytes: &[u8]) -> Vec<u8> {
  let len = bytes.len() as u64;
  let mut buf = Vec::with_capacity(8 + bytes.len() + 1);

  buf.extend_from_slice(&len.to_le_bytes());
  buf.extend_from_slice(bytes);
  buf.push(0);
  buf
}

/// Read the length-prefix at `ptr` and return the borrowed
/// bytes (without the trailing nul). Caller guarantees
/// `ptr` points to a `[len: u64][bytes][null]` buffer.
unsafe fn read_length_prefixed(ptr: *const u8) -> &'static [u8] {
  let len = unsafe { (ptr as *const u64).read_unaligned() } as usize;

  unsafe { slice::from_raw_parts(ptr.add(8), len) }
}

/// One reactive text binding: replace `commands[cmd_idx]`
/// (a `Text`) with the rendered form of `STATE[slot_id]`
/// (`is_str == 0`) or `STR_STATE[slot_id]` (`is_str != 0`).
/// `_pad` reserves a future tag byte without an ABI break.
#[repr(C)]
pub struct TextBinding {
  pub cmd_idx: u32,
  pub slot_id: u32,
  pub is_str: u32,
  pub _pad: u32,
}

/// One reactive list binding in the AOT context. The placeholder
/// `commands[cmd_idx]` is replaced by the rendered elements of
/// reactive array slot `items_slot`, each emitted by walking the
/// per-item recipe at `[recipe_ptr, recipe_ptr + recipe_len)` — a
/// postcard `Vec<UiCommand>` whose item value is a sentinel
/// `Text` (`LIST_ITEM_SENTINEL`). Field order + sizes are ABI:
/// `cmd_idx`@0, `items_slot`@4, `recipe_ptr`@8, `recipe_len`@16
/// (24 bytes, 8-aligned).
#[repr(C)]
pub struct ListBindingAbi {
  pub cmd_idx: u32,
  pub items_slot: u32,
  pub recipe_ptr: *const u8,
  pub recipe_len: usize,
}

/// One reactive attribute binding (`<input value={input_val}>`).
/// The runtime re-applies `commands[cmd_idx].attrs[attr_idx]`'s
/// value from state slot `slot` (`is_str` selects `STR_STATE`
/// over `STATE`) via `UiCommand::set_attr`, so a program-side
/// `input_val = ""` clears the input. Field order + sizes are
/// ABI: `cmd_idx`@0, `attr_idx`@4, `slot`@8, `is_str`@12
/// (16 bytes).
#[repr(C)]
pub struct AttrBindingAbi {
  pub cmd_idx: u32,
  pub attr_idx: u32,
  pub slot: u32,
  pub is_str: u32,
}

/// AOT entry-point context. Built in the compiled exe's
/// stack frame by codegen and passed by reference to
/// `_zo_run_native` at startup.
///
/// Field order is part of the ABI. Optional callback /
/// pointer fields stay null when the program doesn't use
/// the matching feature. New fields may be appended
/// without breaking compiled binaries.
///
/// Reactive state lives **inside the runtime** (a global
/// `Vec<i64>` allocated by `zo_state_init`). Closures
/// mutate it through `zo_state_get` / `zo_state_set` FFI
/// calls — so the context only needs to know about the
/// per-program text-binding map; the state pointer never
/// crosses the ABI.
#[repr(C)]
pub struct ZoRuntimeContext {
  /// Pointer to postcard-encoded `Vec<UiCommand>` (in the
  /// exe's rodata).
  pub template_ptr: *const u8,
  /// Length in bytes of the template payload.
  pub template_len: usize,
  /// Called when a UI event fires. Null = static template
  /// (events silently ignored). `value_ptr` carries the
  /// payload as a zo-format length-prefixed string for
  /// text-bearing events (`@input`/`@change`/`@submit`),
  /// null for click. The pointer is valid for the call
  /// only; persist via `zo_state_set_str` (which copies).
  pub handle_event: Option<
    unsafe extern "C" fn(widget_id: u32, event_kind: u32, value_ptr: *const u8),
  >,
  /// Pointer to an array of `TextBinding` records that
  /// tell the runtime which `commands[cmd_idx]` to refresh
  /// from which `state[slot_id]`. Null = no reactive
  /// bindings (display never updates).
  pub text_bindings_ptr: *const TextBinding,
  /// Number of `TextBinding`s at `text_bindings_ptr`.
  pub text_bindings_count: usize,
  /// Pointer to an array of `ListBindingAbi` records — the
  /// reactive `<X>{arr.map(...)}</X>` lists. Null / count 0 = no
  /// list bindings. Appended after the text fields, so an older
  /// binary that doesn't set them is safe **only** while the
  /// runtime never reads them; `_zo_run_native` reads them
  /// solely when the program ships list bindings.
  pub list_bindings_ptr: *const ListBindingAbi,
  /// Number of `ListBindingAbi`s at `list_bindings_ptr`.
  pub list_bindings_count: usize,
  /// Pointer to an array of `AttrBindingAbi` records — reactive
  /// element attributes (`value={input_val}`). Null / count 0 =
  /// none. Read only when the program ships attr bindings.
  pub attr_bindings_ptr: *const AttrBindingAbi,
  /// Number of `AttrBindingAbi`s at `attr_bindings_ptr`.
  pub attr_bindings_count: usize,
}

/// Send wrapper for a raw pointer or fn pointer. The exe's
/// stack outlives the runtime call, so the pointers stay
/// valid for the whole runtime lifetime; the runtime
/// promises not to retain them past the call.
#[derive(Clone, Copy)]
pub struct SendPtr<T>(pub T);

unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

/// Decode the context's embedded template payload into a
/// command vec. Extracted so unit tests can exercise the
/// decode path without launching a UI runtime.
///
/// # Safety
///
/// The `(template_ptr, template_len)` pair must describe a
/// valid postcard-encoded `Vec<UiCommand>` byte range owned
/// by the caller for the duration of this call.
pub unsafe fn decode_template(
  ctx: &ZoRuntimeContext,
) -> Result<Vec<UiCommand>, CodecError> {
  let bytes =
    unsafe { slice::from_raw_parts(ctx.template_ptr, ctx.template_len) };

  codec::decode(bytes)
}

/// Resize the global state buffers to `count` slots
/// (idempotent, grow-only). `STR_STATE` is sized in
/// lockstep so an early `zo_state_get_str` returns a
/// valid empty-string pointer rather than crashing.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_init(count: u32) {
  let mut state = state().lock().unwrap();

  if state.len() < count as usize {
    state.resize(count as usize, 0);
  }

  let mut str_state = str_state().lock().unwrap();

  if str_state.len() < count as usize {
    str_state.resize_with(count as usize, || encode_length_prefixed(b""));
  }

  let mut arr_state = arr_state().lock().unwrap();

  if arr_state.len() < count as usize {
    arr_state.resize_with(count as usize, Vec::new);
  }

  // Size the dirty set in lockstep (grow-only) so every
  // declared slot has a bit before the first write.
  dirty().lock().unwrap().ensure(count as usize);
}

/// Read state slot `slot`. Returns 0 for out-of-range
/// slots (defensive — codegen should never emit a get
/// outside the `init`-declared range, but a stale binary
/// against a newer runtime should fail soft, not crash).
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_get(slot: u32) -> i64 {
  let state = state().lock().unwrap();

  state.get(slot as usize).copied().unwrap_or(0)
}

/// Write `value` into state slot `slot`. Out-of-range
/// writes are silently dropped (same defensive rationale
/// as `zo_state_get`).
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_set(slot: u32, value: i64) {
  {
    let mut state = state().lock().unwrap();
    let Some(s) = state.get_mut(slot as usize) else {
      return;
    };

    *s = value;
  }

  // Mark dirty only after a successful write — a dropped
  // out-of-range write changes nothing, so it stays clean.
  dirty().lock().unwrap().mark(slot);
}

/// Copy the length-prefixed string at `ptr` into the
/// reactive string slot. The closure body (or main's
/// initialiser) passes the zo-internal `str` pointer
/// directly — the runtime owns the resulting bytes so
/// the buffer stays valid past the closure's frame.
///
/// # Safety
///
/// `ptr` must be either null (silently treated as the
/// empty string) or a zo-format length-prefixed string
/// pointer (`[len: u64][bytes][null]`) that lives for
/// the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_state_set_str(slot: u32, ptr: *const u8) {
  let bytes: &[u8] = if ptr.is_null() {
    b""
  } else {
    unsafe { read_length_prefixed(ptr) }
  };

  let encoded = encode_length_prefixed(bytes);

  {
    let mut str_state = str_state().lock().unwrap();
    let Some(slot_buf) = str_state.get_mut(slot as usize) else {
      return;
    };

    *slot_buf = encoded;
  }

  dirty().lock().unwrap().mark(slot);
}

/// Return a pointer to `slot`'s length-prefixed bytes.
/// The pointer dangles only if a subsequent
/// `zo_state_set_str` overwrites the same slot — same
/// hazard the Rust borrow checker would reject for
/// `let r = &v; v = ...; *r`. Out-of-range reads return a
/// static empty length-prefixed buffer (never null), so
/// codegen can unconditionally dereference.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_get_str(slot: u32) -> *const u8 {
  // Matches `encode_length_prefixed(b"")` byte-for-byte;
  // `empty_static_matches_encoded_empty` enforces lockstep.
  static EMPTY: [u8; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 0];

  let str_state = str_state().lock().unwrap();

  match str_state.get(slot as usize) {
    Some(buf) => buf.as_ptr(),
    None => EMPTY.as_ptr(),
  }
}

/// Push the length-prefixed string at `ptr` onto array slot
/// `slot`, marking the slot dirty. The compiled `todos.push(x)`
/// on a reactive `[]str` lowers to this — the runtime copies the
/// bytes, so they outlive the closure frame, and the dirty mark
/// drives the list re-render.
///
/// # Safety
///
/// `ptr` must be null (treated as the empty string) or a
/// zo-format length-prefixed string pointer (`[len: u64][bytes]
/// [null]`) that lives for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_state_arr_push(slot: u32, ptr: *const u8) {
  let bytes: &[u8] = if ptr.is_null() {
    b""
  } else {
    unsafe { read_length_prefixed(ptr) }
  };
  let item = String::from_utf8_lossy(bytes).into_owned();

  {
    let mut arr_state = arr_state().lock().unwrap();
    let Some(slot_vec) = arr_state.get_mut(slot as usize) else {
      return;
    };

    slot_vec.push(item);
  }

  dirty().lock().unwrap().mark(slot);
}

/// Assign one item of a reactive `[]str` slot in place — the
/// sibling of `zo_state_arr_push` for `todos[i] = v`. The value
/// pointer is a length-prefixed string; an out-of-range index or
/// slot is a no-op (never panic across the FFI boundary). Marks
/// the slot dirty so the bound list re-renders.
///
/// # Safety
///
/// `ptr` must be null or point at a valid length-prefixed string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_state_arr_set(
  slot: u32,
  index: u64,
  ptr: *const u8,
) {
  let bytes: &[u8] = if ptr.is_null() {
    b""
  } else {
    unsafe { read_length_prefixed(ptr) }
  };
  let item = String::from_utf8_lossy(bytes).into_owned();

  {
    let mut arr_state = arr_state().lock().unwrap();
    let Some(slot_vec) = arr_state.get_mut(slot as usize) else {
      return;
    };
    let Some(entry) = slot_vec.get_mut(index as usize) else {
      return;
    };

    *entry = item;
  }

  dirty().lock().unwrap().mark(slot);
}

/// Number of elements in array slot `slot` (0 for an unset /
/// out-of-range slot). The reactive `todos.len` read.
#[unsafe(no_mangle)]
pub extern "C" fn zo_state_arr_len(slot: u32) -> u64 {
  let arr_state = arr_state().lock().unwrap();

  arr_state.get(slot as usize).map_or(0, |v| v.len() as u64)
}

/// Snapshot the items of array slot `slot` for the list
/// re-render — cloned so the renderer never holds the
/// `ARR_STATE` lock while it walks the recipe. Empty for an
/// unset / out-of-range slot.
pub fn arr_slot_items(slot: u32) -> Vec<String> {
  let arr_state = arr_state().lock().unwrap();

  arr_state.get(slot as usize).cloned().unwrap_or_default()
}

/// Render `recipe` once per element of `items`, substituting the
/// element value for each sentinel `Text` placeholder. The
/// per-item building block the list re-render splices over a
/// placeholder command. An empty `items` yields no commands —
/// the list collapses to nothing, as it should.
pub fn render_list(items: &[String], recipe: &[UiCommand]) -> Vec<UiCommand> {
  let mut out = Vec::with_capacity(items.len() * recipe.len());

  for item in items {
    for cmd in recipe {
      match cmd {
        UiCommand::Text(text) if text == LIST_ITEM_SENTINEL => {
          out.push(UiCommand::Text(item.clone()));
        }
        other => out.push(other.clone()),
      }
    }
  }

  out
}

/// A list binding decoded from the AOT context: the placeholder
/// command index, the reactive array slot that drives it, and the
/// per-item recipe (postcard-decoded once at startup). The
/// runtime, post-codegen counterpart of the compile-time
/// `zo_sir::ListBinding` — slot ids and a `UiCommand` recipe
/// instead of symbols and `ListItemCmd`s.
pub struct ListBindingDecoded {
  pub cmd_idx: usize,
  pub items_slot: u32,
  pub recipe: Vec<UiCommand>,
}

/// Decode the context's `ListBindingAbi` array into owned
/// `ListBinding`s, postcard-decoding each recipe once. A binding
/// whose recipe fails to decode is skipped (defensive — a
/// corrupt blob drops that one list, not the whole UI).
///
/// # Safety
///
/// `ctx.list_bindings_ptr` must be null or point to
/// `ctx.list_bindings_count` valid `ListBindingAbi` entries whose
/// `(recipe_ptr, recipe_len)` ranges are valid for the call.
pub unsafe fn decode_list_bindings(
  ctx: &ZoRuntimeContext,
) -> Vec<ListBindingDecoded> {
  if ctx.list_bindings_ptr.is_null() {
    return Vec::new();
  }

  let abis = unsafe {
    slice::from_raw_parts(ctx.list_bindings_ptr, ctx.list_bindings_count)
  };
  let mut out = Vec::with_capacity(abis.len());

  for abi in abis {
    let bytes =
      unsafe { slice::from_raw_parts(abi.recipe_ptr, abi.recipe_len) };

    if let Ok(recipe) = codec::decode(bytes) {
      out.push(ListBindingDecoded {
        cmd_idx: abi.cmd_idx as usize,
        items_slot: abi.items_slot,
        recipe,
      });
    }
  }

  out
}

/// One attribute binding decoded from the AOT context: the
/// `Element` command, the attribute index within it, and the
/// state slot driving its value.
pub struct AttrBindingDecoded {
  pub cmd_idx: usize,
  pub attr_idx: usize,
  pub slot: u32,
  pub is_str: bool,
}

/// Decode the context's `AttrBindingAbi` array. Null pointer →
/// no bindings.
///
/// # Safety
///
/// `ctx.attr_bindings_ptr` must be null or point to
/// `ctx.attr_bindings_count` valid `AttrBindingAbi` entries.
pub unsafe fn decode_attr_bindings(
  ctx: &ZoRuntimeContext,
) -> Vec<AttrBindingDecoded> {
  if ctx.attr_bindings_ptr.is_null() {
    return Vec::new();
  }

  let abis = unsafe {
    slice::from_raw_parts(ctx.attr_bindings_ptr, ctx.attr_bindings_count)
  };

  abis
    .iter()
    .map(|abi| AttrBindingDecoded {
      cmd_idx: abi.cmd_idx as usize,
      attr_idx: abi.attr_idx as usize,
      slot: abi.slot,
      is_str: abi.is_str != 0,
    })
    .collect()
}

/// Re-apply each attribute binding whose slot passes `should_fire`
/// — read the slot's current value and `set_attr` it on the bound
/// element's attribute. The element already carries the attr name
/// (it's `commands[cmd_idx].attrs[attr_idx]`), so no name blob
/// crosses the ABI. A program-side `input_val = ""` clears the
/// input this way.
fn apply_attr_bindings(
  cmds: &mut [UiCommand],
  attrs: &[AttrBindingDecoded],
  should_fire: impl Fn(u32) -> bool,
) {
  for attr in attrs {
    if !should_fire(attr.slot) {
      continue;
    }

    let Some(value) = state_slot_text(attr.slot, attr.is_str) else {
      continue;
    };

    // Resolve the attr name (immutable peek), then set it.
    let name = match cmds.get(attr.cmd_idx) {
      Some(UiCommand::Element { attrs: a, .. }) => {
        a.get(attr.attr_idx).map(|x| x.name().to_string())
      }
      _ => None,
    };

    if let Some(name) = name
      && let Some(cmd) = cmds.get_mut(attr.cmd_idx)
    {
      cmd.set_attr(&name, &value);
    }
  }
}

/// Rebuild the command stream from the static `base` template:
/// bake every text binding from the global state, re-apply every
/// attribute binding, then splice each list's rendered items over
/// its placeholder. Used for the initial frame and whenever a list
/// slot changed — the splice shifts tail indices, so the fine-
/// grained in-place path can't apply there.
///
/// Lists splice front-to-back with a running offset (mirrors the
/// `zo run` list applier); the current template shapes carry no
/// binding past a list anchor, so a single forward pass is exact.
///
/// # Safety
///
/// `text_bindings_ptr` must be null or point to
/// `text_bindings_count` valid `TextBinding` entries.
pub unsafe fn rebuild_with_lists(
  base: &[UiCommand],
  text_bindings_ptr: *const TextBinding,
  text_bindings_count: usize,
  attrs: &[AttrBindingDecoded],
  lists: &[ListBindingDecoded],
) -> Vec<UiCommand> {
  let mut cmds = base.to_vec();

  unsafe {
    refresh_bindings_from_global(
      text_bindings_ptr,
      text_bindings_count,
      &mut cmds,
    );
  }

  // Attr bindings apply before the splice — they target the base
  // commands (e.g. the `<input>`), all of which sit before any
  // list anchor in the current shapes.
  apply_attr_bindings(&mut cmds, attrs, |_| true);

  let mut offset: isize = 0;

  for list in lists {
    let target = (list.cmd_idx as isize + offset) as usize;

    if target >= cmds.len() {
      continue;
    }

    let items = arr_slot_items(list.items_slot);
    let rendered = render_list(&items, &list.recipe);
    let new_len = rendered.len();

    cmds.splice(target..target + 1, rendered);
    offset += new_len as isize - 1;
  }

  cmds
}

/// Refresh every reactive `commands[cmd_idx]` from
/// `state` / `str_state`. Pure inner that takes state
/// explicitly — `refresh_bindings_from_global` wraps it
/// for production while tests inject deterministic state.
///
/// Safety: `bindings_ptr` must point to `bindings_count`
/// valid `TextBinding` entries.
unsafe fn refresh_bindings(
  state: &[i64],
  str_state: &[Vec<u8>],
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  if bindings_ptr.is_null() {
    return;
  }

  let bindings = unsafe { slice::from_raw_parts(bindings_ptr, bindings_count) };

  for binding in bindings {
    let slot = binding.slot_id as usize;
    let cmd_idx = binding.cmd_idx as usize;

    if cmd_idx >= commands.len() {
      continue;
    }

    let UiCommand::Text(target) = &mut commands[cmd_idx] else {
      continue;
    };

    if binding.is_str != 0 {
      let buf = match str_state.get(slot) {
        Some(b) if b.len() >= 8 => b,
        _ => continue,
      };
      let len = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;

      if 8 + len > buf.len() {
        continue;
      }

      let new_bytes = &buf[8..8 + len];

      // Bytes-equal short-circuit avoids the
      // `String::from_utf8_lossy` allocation on the no-op
      // path — for steady-state UI loops refresh fires
      // every frame and most slots are unchanged.
      if target.as_bytes() == new_bytes {
        continue;
      }

      *target = String::from_utf8_lossy(new_bytes).into_owned();
    } else {
      let value = match state.get(slot) {
        Some(v) => *v,
        None => continue,
      };
      let new_text = value.to_string();

      if *target != new_text {
        *target = new_text;
      }
    }
  }
}

/// Production wrapper — holds both reactive-state mutexes
/// across the binding walk and forwards to
/// `refresh_bindings`.
///
/// # Safety
///
/// `bindings_ptr` must be null or point to `bindings_count`
/// valid `TextBinding` entries that live for the call.
pub unsafe fn refresh_bindings_from_global(
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
  commands: &mut [UiCommand],
) {
  let state = state().lock().unwrap();
  let str_state = str_state().lock().unwrap();

  unsafe {
    refresh_bindings(&state, &str_state, bindings_ptr, bindings_count, commands)
  };
}

/// Decode the `TextBinding` array into the reverse index the
/// per-event refresh walks (state slot → the `Text` commands it
/// drives) plus the per-slot string-typed flag. Built once at
/// registry time; an empty / null array yields an empty graph.
///
/// # Safety
///
/// `bindings_ptr` must be null or point to `bindings_count`
/// valid `TextBinding` entries that live for the call.
unsafe fn build_text_binding_graph(
  bindings_ptr: *const TextBinding,
  bindings_count: usize,
) -> (BindingGraph, Vec<bool>) {
  if bindings_ptr.is_null() {
    return (BindingGraph::default(), Vec::new());
  }

  let bindings = unsafe { slice::from_raw_parts(bindings_ptr, bindings_count) };
  let mut edges = Vec::with_capacity(bindings_count);
  // `is_str[slot]` selects `STR_STATE` over `STATE` when reading
  // the slot's text — a slot's type is fixed, so this is keyed
  // by slot, not by binding.
  let mut is_str = Vec::new();

  for binding in bindings {
    edges.push((
      binding.slot_id,
      BindingRef::Text {
        cmd_idx: binding.cmd_idx,
      },
    ));

    let slot = binding.slot_id as usize;

    if slot >= is_str.len() {
      is_str.resize(slot + 1, false);
    }

    is_str[slot] = binding.is_str != 0;
  }

  (BindingGraph::from_edges(is_str.len(), &edges), is_str)
}

/// The display string of state `slot` — the decimal of
/// `STATE[slot]` for a scalar, or the UTF-8 of `STR_STATE[slot]`
/// for a string slot. `None` for an out-of-range scalar slot;
/// a malformed string buffer reads as empty. The value source
/// the per-event `refresh_dirty` reads.
fn state_slot_text(slot: u32, is_str: bool) -> Option<String> {
  if is_str {
    let str_state = str_state().lock().unwrap();
    let buf = str_state.get(slot as usize)?;

    if buf.len() < 8 {
      return Some(String::new());
    }

    let len = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;

    if 8 + len > buf.len() {
      return Some(String::new());
    }

    Some(String::from_utf8_lossy(&buf[8..8 + len]).into_owned())
  } else {
    let state = state().lock().unwrap();

    Some(state.get(slot as usize)?.to_string())
  }
}

/// Per-program inputs threaded into [`build_registry`] beyond the
/// dispatcher and shared buffer — grouped so the call stays
/// within the argument budget.
/// What the last event's refresh did to the shared stream. A
/// retained-mode view layer (iOS) patches exactly this instead of
/// re-diffing the whole stream; immediate-mode targets ignore it.
#[derive(Default)]
pub struct UpdateReport {
  /// Command indices the refresh rewrote in place.
  pub touched: DirtyCommands,
  /// The refresh replaced the stream wholesale (a list write
  /// changed the item count), so indices shifted — the view must
  /// rebuild until it consumes `list_edits`.
  pub structural: bool,
  /// Set when the structural change is a pure tail append on the
  /// template's single list (the push case): the command block
  /// `[at, at + count)` in the rebuilt stream is new, everything
  /// before it is unchanged. A view layer can build just those
  /// items instead of rebuilding.
  pub appended: Option<AppendedItems>,
  /// Keyed edit script per written list (list index in the
  /// decoded order, edits from [`reconcile_list`]). A
  /// same-length write reports no structural flag — its regions
  /// land in `touched` instead.
  pub list_edits: Vec<(usize, Vec<ListEdit>)>,
}

/// A pure tail append's command block — see
/// [`UpdateReport::appended`].
#[derive(Clone, Copy, Debug)]
pub struct AppendedItems {
  /// First command index of the appended block.
  pub at: usize,
  /// How many commands the block spans.
  pub count: usize,
}

pub struct RegistryInputs {
  /// The static template (placeholders unbaked). The dedup walk
  /// reads it for `Event` commands, and the handler rebuilds the
  /// command stream from it whenever a list slot changes.
  pub base: Vec<UiCommand>,
  /// Decoded list bindings (empty for a list-free template).
  pub lists: Vec<ListBindingDecoded>,
  /// Decoded attribute bindings (empty when none).
  pub attrs: Vec<AttrBindingDecoded>,
  /// The `TextBinding` array pointer + count.
  pub bindings_ptr: SendPtr<*const TextBinding>,
  pub bindings_count: usize,
  /// Per-event update report the view layer reads after dispatch.
  pub report: Arc<Mutex<UpdateReport>>,
}

/// Build an `EventRegistry` whose callbacks dispatch through
/// `ctx.handle_event` AND, after the dispatcher returns, refresh
/// only the reactive bindings whose state slots the handler
/// actually wrote. Walks the base template left-to-right,
/// assigns each unique handler name a sequential `u32` index —
/// the codegen-side dispatcher uses the same dedupe-by-first-
/// seen-name scheme, so the indices line up automatically.
///
/// The `TextBinding` array is folded once into a [`BindingGraph`]
/// reverse index; each handler then drains the dirty set written
/// by `zo_state_set` and refreshes only those slots' commands —
/// O(written), not O(all bindings). A write to a list's items
/// slot instead rebuilds the whole stream (the splice shifts
/// indices, so the fine-grained path can't apply there).
pub fn build_registry(
  dispatch: SendPtr<unsafe extern "C" fn(u32, u32, *const u8)>,
  shared_cmds: Arc<Mutex<Vec<UiCommand>>>,
  inputs: RegistryInputs,
) -> EventRegistry {
  let RegistryInputs {
    base,
    lists,
    attrs,
    bindings_ptr,
    bindings_count,
    report,
  } = inputs;
  let mut registry = EventRegistry::new();

  // Last-rendered item keys per list, diffed against the next
  // write. Seeded from the current state so the first event diffs
  // against the initial render, not an empty list.
  let list_keys: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(
    lists
      .iter()
      .map(|list| arr_slot_items(list.items_slot))
      .collect(),
  ));
  let mut seen: HashSet<String> = HashSet::new();
  let mut handler_idx: u32 = 0;

  // One reverse index for every handler — the binding array is
  // immutable for the program's life, so build it once.
  let (graph, is_str) =
    unsafe { build_text_binding_graph(bindings_ptr.0, bindings_count) };
  let graph = Arc::new(graph);
  let is_str = Arc::new(is_str);
  let base = Arc::new(base);
  let lists = Arc::new(lists);
  let attrs = Arc::new(attrs);

  for cmd in base.iter() {
    let UiCommand::Event {
      handler,
      event_kind,
      ..
    } = cmd
    else {
      continue;
    };

    if !seen.insert(handler.clone()) {
      continue;
    }

    let idx = handler_idx;
    let kind_u32 = *event_kind as u32;
    // Capture `SendPtr` directly — the wrapper is Send, the bare
    // fn pointer isn't. The graph / is_str / base / lists are
    // plain `Arc`s (Send + Sync), so they cross in freely.
    let dispatch_send = dispatch;
    let bindings_send = bindings_ptr;
    let cmds_arc = Arc::clone(&shared_cmds);
    let graph = Arc::clone(&graph);
    let is_str = Arc::clone(&is_str);
    let base = Arc::clone(&base);
    let lists = Arc::clone(&lists);
    let attrs = Arc::clone(&attrs);
    let report = Arc::clone(&report);
    let list_keys = Arc::clone(&list_keys);
    let cb: EventHandler = Box::new(move |payload| {
      // RFC 2229 disjoint captures would otherwise pull
      // `dispatch_send.0` / `bindings_send.0` directly into the
      // closure type, defeating `SendPtr`'s wrapper-level `Send`.
      // Reference each binding whole to force whole captures.
      let _ = (&dispatch_send, &bindings_send);

      // `event_holder` IS the `Event { value: str }` struct
      // — one 8-byte field holding a pointer to the
      // length-prefixed payload buffer. Both live on this
      // stack frame for the call's duration; the closure
      // copies via `zo_state_set_str` if it wants to keep
      // the value past the call.
      let payload_buf = encode_length_prefixed(payload.value.as_bytes());
      let event_holder: u64 = payload_buf.as_ptr() as u64;

      // Dispatcher's `mov x1, x2` forwards `&event_holder`
      // into the closure's `param[1]`, then tail-calls.
      unsafe {
        (dispatch_send.0)(
          idx,
          kind_u32,
          &event_holder as *const u64 as *const u8,
        )
      };

      // The dispatcher's `zo_state_set` / `zo_state_arr_push`
      // calls marked every written slot. Drain them: a write to a
      // list's items slot reshapes the stream (rebuild from
      // base); otherwise patch only the dirtied text/attr
      // commands in place — a no-op write leaves them identical.
      let mut written = Vec::new();

      drain_dirty(&mut written);

      let hits_list = lists.iter().any(|l| written.contains(&l.items_slot));
      let mut cmds = cmds_arc.lock().unwrap();

      if hits_list {
        // Keyed diff per written list: same item count means the
        // stream keeps its shape (fixed stride per item), so the
        // write patches in place — only the changed commands mark
        // touched and the view skips the rebuild. A length change
        // shifts every index after the splice; report it
        // structural and carry the edit script for view layers
        // that can apply it.
        let mut keys = list_keys.lock().unwrap();
        let mut edits = Vec::new();
        let mut shape_kept = true;

        for (list_idx, list) in lists.iter().enumerate() {
          if !written.contains(&list.items_slot) {
            continue;
          }

          let new_items = arr_slot_items(list.items_slot);
          let old_items = keys.get(list_idx).cloned().unwrap_or_default();

          if old_items.len() != new_items.len() {
            shape_kept = false;
          }

          edits.push((list_idx, reconcile_list(&old_items, &new_items)));

          if list_idx >= keys.len() {
            keys.resize(list_idx + 1, Vec::new());
          }

          keys[list_idx] = new_items;
        }

        let rebuilt = unsafe {
          rebuild_with_lists(
            base.as_slice(),
            bindings_send.0,
            bindings_count,
            attrs.as_slice(),
            lists.as_slice(),
          )
        };

        let mut report = report.lock().unwrap();

        // The push fast path: one list, all edits tail inserts,
        // existing items to anchor on — the appended command
        // block is everything past the old region end.
        report.appended =
          if lists.len() == 1 && edits.len() == 1 && !keys.is_empty() {
            let stride = lists[0].recipe.len();
            let new_count = keys[0].len();
            let inserts = edits[0].1.len();
            let old_count = new_count - inserts.min(new_count);
            let tail_only = old_count > 0
              && edits[0].1.iter().enumerate().all(|(offset, edit)| {
                matches!(edit, ListEdit::Insert { to }
                if *to as usize == old_count + offset)
              });

            tail_only.then(|| AppendedItems {
              at: lists[0].cmd_idx + old_count * stride,
              count: inserts * stride,
            })
          } else {
            None
          };

        report.list_edits = edits;

        if shape_kept && rebuilt.len() == cmds.len() {
          report.structural = false;
          report.touched.clear();
          report.touched.ensure(rebuilt.len());

          for (idx, (old, new)) in cmds.iter().zip(&rebuilt).enumerate() {
            if old != new {
              report.touched.mark(idx as u32);
            }
          }
        } else {
          report.structural = true;
          report.touched.clear();
        }

        *cmds = rebuilt;
      } else {
        let mut touched = DirtyCommands::with_capacity(cmds.len());

        refresh_dirty(&graph, &written, &[], &mut cmds, &mut touched, |slot| {
          state_slot_text(
            slot,
            is_str.get(slot as usize).copied().unwrap_or(false),
          )
        });

        // Re-apply attribute bindings whose slot the handler
        // wrote (e.g. `value={input_val}` after `input_val = ""`).
        apply_attr_bindings(&mut cmds, attrs.as_slice(), |slot| {
          written.contains(&slot)
        });

        // Attr-bound commands count as touched: the view re-reads
        // their attrs even though no text moved.
        for binding in attrs.iter() {
          if written.contains(&binding.slot) {
            touched.mark(binding.cmd_idx as u32);
          }
        }

        let mut report = report.lock().unwrap();

        report.structural = false;
        report.appended = None;
        report.touched = touched;
      }
    });

    registry.register(handler.clone(), cb);
    handler_idx += 1;
  }

  registry
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::render::EventPayload;

  use zo_ui_protocol::{ElementTag, EventKind, UiCommand};

  /// Serializes tests that touch the process-global state cells.
  /// `drain_dirty` empties the WHOLE dirty set, so two tests
  /// draining in parallel steal each other's marks — unique slot
  /// ids alone can't prevent that.
  fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn empty_ctx_with_template(bytes: *const u8, len: usize) -> ZoRuntimeContext {
    ZoRuntimeContext {
      template_ptr: bytes,
      template_len: len,
      handle_event: None,
      text_bindings_ptr: std::ptr::null(),
      text_bindings_count: 0,
      list_bindings_ptr: std::ptr::null(),
      list_bindings_count: 0,
      attr_bindings_ptr: std::ptr::null(),
      attr_bindings_count: 0,
    }
  }

  #[test]
  fn empty_static_matches_encoded_empty() {
    // The out-of-range fallback in `zo_state_get_str` is
    // hand-rolled (`[u8; 9]` of zeros) instead of going
    // through `encode_length_prefixed(b"")` so the function
    // can return a `'static` pointer without `OnceLock`.
    // Lockstep with the encoder via this assertion — if
    // the prefix layout ever changes (e.g. 4-byte length)
    // the static silently desyncs without it.
    let encoded = encode_length_prefixed(b"");

    let fallback = unsafe {
      slice::from_raw_parts(zo_state_get_str(u32::MAX), encoded.len())
    };

    assert_eq!(fallback, encoded.as_slice());
  }

  #[test]
  fn text_binding_layout_matches_codegen_pack() {
    // Codegen serialises 16 bytes per entry: cmd_idx@0,
    // slot_id@4, is_str@8, _pad@12. If this drifts, the
    // binding pointer the runtime decodes will misalign
    // and the wrong slot/flag will land per binding.
    use std::mem::{align_of, offset_of, size_of};

    assert_eq!(size_of::<TextBinding>(), 16);
    assert_eq!(align_of::<TextBinding>(), 4);
    assert_eq!(offset_of!(TextBinding, cmd_idx), 0);
    assert_eq!(offset_of!(TextBinding, slot_id), 4);
    assert_eq!(offset_of!(TextBinding, is_str), 8);
    assert_eq!(offset_of!(TextBinding, _pad), 12);
  }

  #[test]
  fn decode_template_round_trip() {
    let cmds = vec![
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("hello".into()),
      UiCommand::EndElement,
    ];

    let bytes = codec::encode(&cmds).unwrap();
    let ctx = empty_ctx_with_template(bytes.as_ptr(), bytes.len());

    let decoded = unsafe { decode_template(&ctx) }.unwrap();

    assert_eq!(decoded, cmds);
  }

  #[test]
  fn decode_template_rejects_garbage() {
    // Random bytes don't encode any command stream.
    let garbage = [0xFFu8, 0xFE, 0xFD, 0xFC];
    let ctx = empty_ctx_with_template(garbage.as_ptr(), garbage.len());

    assert!(unsafe { decode_template(&ctx) }.is_err());
  }

  #[test]
  fn refresh_bindings_replaces_text_from_state() {
    let _serial = state_lock();
    let state = [42i64];
    let str_state: Vec<Vec<u8>> = vec![vec![]];
    let mut cmds = vec![
      UiCommand::Text("0".into()),
      UiCommand::Text("ignored".into()),
    ];
    let bindings = [TextBinding {
      cmd_idx: 0,
      slot_id: 0,
      is_str: 0,
      _pad: 0,
    }];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    assert_eq!(cmds[0], UiCommand::Text("42".into()));
    // Untouched binding leaves the second Text alone.
    assert_eq!(cmds[1], UiCommand::Text("ignored".into()));
  }

  #[test]
  fn refresh_bindings_replaces_text_from_str_state() {
    let _serial = state_lock();
    let state: [i64; 1] = [0];
    let str_state: Vec<Vec<u8>> = vec![encode_length_prefixed(b"hello")];
    let mut cmds = vec![UiCommand::Text("".into())];
    let bindings = [TextBinding {
      cmd_idx: 0,
      slot_id: 0,
      is_str: 1,
      _pad: 0,
    }];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    assert_eq!(cmds[0], UiCommand::Text("hello".into()));
  }

  #[test]
  fn refresh_bindings_handles_null_pointers() {
    let state: [i64; 0] = [];
    let str_state: Vec<Vec<u8>> = vec![];
    let mut cmds = vec![UiCommand::Text("safe".into())];

    unsafe {
      refresh_bindings(&state, &str_state, std::ptr::null(), 0, &mut cmds)
    };

    assert_eq!(cmds[0], UiCommand::Text("safe".into()));
  }

  #[test]
  fn refresh_bindings_skips_oob_indices() {
    let state = [1i64];
    let str_state: Vec<Vec<u8>> = vec![vec![]];
    let mut cmds = vec![UiCommand::Text("only".into())];
    let bindings = [
      // Out-of-range cmd_idx — should be skipped.
      TextBinding {
        cmd_idx: 5,
        slot_id: 0,
        is_str: 0,
        _pad: 0,
      },
      // Out-of-range slot_id — should be skipped.
      TextBinding {
        cmd_idx: 0,
        slot_id: 10,
        is_str: 0,
        _pad: 0,
      },
    ];

    unsafe {
      refresh_bindings(
        &state,
        &str_state,
        bindings.as_ptr(),
        bindings.len(),
        &mut cmds,
      );
    }

    // Neither binding applied; original content preserved.
    assert_eq!(cmds[0], UiCommand::Text("only".into()));
  }

  #[test]
  fn state_set_get_str_round_trip() {
    let _serial = state_lock();
    zo_state_init(202);

    let payload = encode_length_prefixed(b"world");

    unsafe { zo_state_set_str(201, payload.as_ptr()) };

    let p = zo_state_get_str(201);

    let bytes = unsafe { read_length_prefixed(p) };

    assert_eq!(bytes, b"world");
    // Out-of-range reads return the static empty buffer
    // (length 0) rather than null/segfaulting.
    let q = zo_state_get_str(99999);
    let bytes = unsafe { read_length_prefixed(q) };

    assert_eq!(bytes, b"");
  }

  #[test]
  fn state_get_set_round_trip() {
    let _serial = state_lock();
    // Slot 100 to avoid contention with other tests
    // using low slots (cargo runs tests in parallel and
    // STATE is a process-global `OnceLock<Mutex<Vec>>`).
    zo_state_init(101);
    zo_state_set(100, 99);

    assert_eq!(zo_state_get(100), 99);
    // Out-of-range reads return 0, never panic.
    assert_eq!(zo_state_get(9999), 0);
  }

  #[test]
  fn state_set_marks_dirty_and_drain_clears() {
    let _serial = state_lock();
    // Slot 305 is unique to this test, so no parallel test
    // marks it: the global DIRTY set is process-wide, but
    // only this test touches 305 — assertions stay robust.
    zo_state_init(400);

    let mut drained = Vec::new();
    // Clear whatever earlier writes in this test process left.
    drain_dirty(&mut drained);
    drained.clear();

    zo_state_set(305, 7);
    zo_state_set_str_slot(306, b"hi");

    drain_dirty(&mut drained);

    assert!(drained.contains(&305), "scalar write marks its slot");
    assert!(drained.contains(&306), "string write marks its slot");

    // Drain cleared the set: a second drain no longer reports
    // 305/306 (no test re-marks those slots).
    let mut again = Vec::new();
    drain_dirty(&mut again);

    assert!(!again.contains(&305), "drain clears the dirty bit");
    assert!(!again.contains(&306), "drain clears the dirty bit");
  }

  #[test]
  fn arr_state_push_len_items_and_dirty() {
    let _serial = state_lock();
    // Slot 410 is unique to this test — the global ARR_STATE is
    // process-wide, but nothing else touches 410.
    zo_state_init(420);

    let mut scratch = Vec::new();
    drain_dirty(&mut scratch);
    scratch.clear();

    let a = encode_length_prefixed(b"alpha");
    let b = encode_length_prefixed(b"beta");

    unsafe {
      zo_state_arr_push(410, a.as_ptr());
      zo_state_arr_push(410, b.as_ptr());
    }

    assert_eq!(zo_state_arr_len(410), 2);
    assert_eq!(
      arr_slot_items(410),
      vec!["alpha".to_string(), "beta".to_string()]
    );

    drain_dirty(&mut scratch);
    assert!(scratch.contains(&410), "array push marks its slot dirty");

    // Out-of-range array ops are safe no-ops.
    unsafe { zo_state_arr_push(999_999, a.as_ptr()) };
    assert_eq!(zo_state_arr_len(999_999), 0);
    assert!(arr_slot_items(999_999).is_empty());
  }

  #[test]
  fn render_list_substitutes_sentinel_per_item() {
    use zo_ui_protocol::LIST_ITEM_SENTINEL;

    // Recipe for `<li>{t}</li>`.
    let li = || UiCommand::Element {
      tag: ElementTag::Li,
      attrs: vec![],
      self_closing: false,
    };
    let recipe = vec![
      li(),
      UiCommand::Text(LIST_ITEM_SENTINEL.to_string()),
      UiCommand::EndElement,
    ];

    let items = vec!["a".to_string(), "b".to_string()];

    assert_eq!(
      render_list(&items, &recipe),
      vec![
        li(),
        UiCommand::Text("a".into()),
        UiCommand::EndElement,
        li(),
        UiCommand::Text("b".into()),
        UiCommand::EndElement,
      ]
    );

    // Empty array → the list collapses to nothing.
    assert!(render_list(&[], &recipe).is_empty());
  }

  #[test]
  fn list_binding_abi_layout_matches_codegen_pack() {
    // Codegen emits 24 bytes per entry: cmd_idx@0, items_slot@4,
    // recipe_ptr@8, recipe_len@16. If this drifts the runtime
    // decodes the wrong slot / a garbage recipe pointer.
    use std::mem::{align_of, offset_of, size_of};

    assert_eq!(size_of::<ListBindingAbi>(), 24);
    assert_eq!(align_of::<ListBindingAbi>(), 8);
    assert_eq!(offset_of!(ListBindingAbi, cmd_idx), 0);
    assert_eq!(offset_of!(ListBindingAbi, items_slot), 4);
    assert_eq!(offset_of!(ListBindingAbi, recipe_ptr), 8);
    assert_eq!(offset_of!(ListBindingAbi, recipe_len), 16);
  }

  #[test]
  fn arr_state_set_replaces_item_and_marks_dirty() {
    let _serial = state_lock();

    zo_state_init(480);

    let a = encode_length_prefixed(b"a");
    let b = encode_length_prefixed(b"b");
    let z = encode_length_prefixed(b"z");

    unsafe {
      zo_state_arr_push(475, a.as_ptr());
      zo_state_arr_push(475, b.as_ptr());
    }

    let mut scratch = Vec::new();

    drain_dirty(&mut scratch);

    unsafe { zo_state_arr_set(475, 0, z.as_ptr()) };

    assert_eq!(arr_slot_items(475), vec!["z", "b"]);

    let mut drained = Vec::new();

    drain_dirty(&mut drained);

    assert!(drained.contains(&475), "item write marks the slot dirty");

    // Out-of-range index and slot are no-ops, never panics.
    unsafe { zo_state_arr_set(475, 99, z.as_ptr()) };
    unsafe { zo_state_arr_set(999_999, 0, z.as_ptr()) };

    assert_eq!(arr_slot_items(475), vec!["z", "b"]);
  }

  #[test]
  fn list_write_reports_keyed_edit_script() {
    let _serial = state_lock();

    zo_state_init(470);

    // Seed slot 460 = ["a", "b"] before the registry builds, so
    // the key cache starts from the initial render.
    let a = encode_length_prefixed(b"a");
    let b = encode_length_prefixed(b"b");

    unsafe {
      zo_state_arr_push(460, a.as_ptr());
      zo_state_arr_push(460, b.as_ptr());
    }

    let base = vec![
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__push".into(),
      },
      li(),
    ];

    let report = Arc::new(Mutex::new(UpdateReport::default()));
    let shared = Arc::new(Mutex::new(base.clone()));
    let registry = build_registry(
      SendPtr(noop_dispatch as _),
      Arc::clone(&shared),
      RegistryInputs {
        base,
        lists: vec![ListBindingDecoded {
          cmd_idx: 1,
          items_slot: 460,
          recipe: li_recipe(),
        }],
        attrs: Vec::new(),
        bindings_ptr: SendPtr(std::ptr::null()),
        bindings_count: 0,
        report: Arc::clone(&report),
      },
    );

    // Clear the seeding pushes' dirty marks — a real program's
    // initial render drains before any event.
    let mut scratch = Vec::new();

    drain_dirty(&mut scratch);

    // The "tap": the handler pushes "c" (the noop dispatcher runs
    // no zo code, so the test performs the handler's write).
    let c = encode_length_prefixed(b"c");

    unsafe { zo_state_arr_push(460, c.as_ptr()) };
    registry.dispatch("__push", &EventPayload::default());

    {
      let report = report.lock().unwrap();

      assert!(report.structural, "a push changes the item count");
      assert_eq!(
        report.list_edits,
        vec![(0, vec![ListEdit::Insert { to: 2 }])],
        "push diffs to one insert at the tail"
      );
    }

    // Second event: the cache moved with the last write, so the
    // next push diffs against [a, b, c], not the seed.
    let d = encode_length_prefixed(b"d");

    unsafe { zo_state_arr_push(460, d.as_ptr()) };
    registry.dispatch("__push", &EventPayload::default());

    let report = report.lock().unwrap();

    assert_eq!(
      report.list_edits,
      vec![(0, vec![ListEdit::Insert { to: 3 }])],
      "the key cache advances with each write"
    );

    // The stream itself grew by one recipe stride per push.
    let cmds = shared.lock().unwrap();
    let li_count = cmds
      .iter()
      .filter(
        |c| matches!(c, UiCommand::Element { tag, .. } if *tag == ElementTag::Li),
      )
      .count();

    assert_eq!(li_count, 4, "four items rendered after two pushes");
  }

  fn li() -> UiCommand {
    UiCommand::Element {
      tag: ElementTag::Li,
      attrs: vec![],
      self_closing: false,
    }
  }

  /// `<li>{t}</li>` recipe with the per-item sentinel.
  fn li_recipe() -> Vec<UiCommand> {
    use zo_ui_protocol::LIST_ITEM_SENTINEL;

    vec![
      li(),
      UiCommand::Text(LIST_ITEM_SENTINEL.to_string()),
      UiCommand::EndElement,
    ]
  }

  #[test]
  fn decode_list_bindings_round_trip() {
    let recipe = li_recipe();
    let recipe_bytes = codec::encode(&recipe).unwrap();
    let abis = [ListBindingAbi {
      cmd_idx: 3,
      items_slot: 7,
      recipe_ptr: recipe_bytes.as_ptr(),
      recipe_len: recipe_bytes.len(),
    }];
    let template = codec::encode(&[UiCommand::Text("x".into())]).unwrap();
    let ctx = ZoRuntimeContext {
      template_ptr: template.as_ptr(),
      template_len: template.len(),
      handle_event: None,
      text_bindings_ptr: std::ptr::null(),
      text_bindings_count: 0,
      list_bindings_ptr: abis.as_ptr(),
      list_bindings_count: abis.len(),
      attr_bindings_ptr: std::ptr::null(),
      attr_bindings_count: 0,
    };

    let decoded = unsafe { decode_list_bindings(&ctx) };

    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].cmd_idx, 3);
    assert_eq!(decoded[0].items_slot, 7);
    assert_eq!(decoded[0].recipe, recipe);

    // Null list pointer → no bindings.
    let empty = empty_ctx_with_template(template.as_ptr(), template.len());
    assert!(unsafe { decode_list_bindings(&empty) }.is_empty());
  }

  #[test]
  fn rebuild_with_lists_splices_array_items() {
    let _serial = state_lock();
    // Array slot 420 is unique to this test.
    zo_state_init(430);

    unsafe {
      zo_state_arr_push(420, encode_length_prefixed(b"buy milk").as_ptr());
      zo_state_arr_push(420, encode_length_prefixed(b"walk dog").as_ptr());
    }

    let ul = || UiCommand::Element {
      tag: ElementTag::Ul,
      attrs: vec![],
      self_closing: false,
    };
    // `<ul>{placeholder}</ul>` — the list anchor at index 1.
    let base =
      vec![ul(), UiCommand::Text(String::new()), UiCommand::EndElement];
    let lists = vec![ListBindingDecoded {
      cmd_idx: 1,
      items_slot: 420,
      recipe: li_recipe(),
    }];

    let rebuilt =
      unsafe { rebuild_with_lists(&base, std::ptr::null(), 0, &[], &lists) };

    assert_eq!(
      rebuilt,
      vec![
        ul(),
        li(),
        UiCommand::Text("buy milk".into()),
        UiCommand::EndElement,
        li(),
        UiCommand::Text("walk dog".into()),
        UiCommand::EndElement,
        UiCommand::EndElement,
      ]
    );
  }

  #[test]
  fn attr_binding_abi_layout_matches_codegen_pack() {
    // Codegen emits 16 bytes per entry: cmd_idx@0, attr_idx@4,
    // slot@8, is_str@12.
    use std::mem::{align_of, offset_of, size_of};

    assert_eq!(size_of::<AttrBindingAbi>(), 16);
    assert_eq!(align_of::<AttrBindingAbi>(), 4);
    assert_eq!(offset_of!(AttrBindingAbi, cmd_idx), 0);
    assert_eq!(offset_of!(AttrBindingAbi, attr_idx), 4);
    assert_eq!(offset_of!(AttrBindingAbi, slot), 8);
    assert_eq!(offset_of!(AttrBindingAbi, is_str), 12);
  }

  #[test]
  fn apply_attr_bindings_sets_value_from_state() {
    let _serial = state_lock();
    use zo_ui_protocol::{Attr, PropValue};

    // Slot 430 = the input's `value` (a string), set to "typed".
    zo_state_init(440);
    zo_state_set_str_slot(430, b"typed");

    let mut cmds = vec![UiCommand::Element {
      tag: ElementTag::Input,
      attrs: vec![Attr::Dynamic {
        name: "value".into(),
        var: 0,
        initial: PropValue::Str(String::new()),
      }],
      self_closing: true,
    }];
    let attrs = vec![AttrBindingDecoded {
      cmd_idx: 0,
      attr_idx: 0,
      slot: 430,
      is_str: true,
    }];

    apply_attr_bindings(&mut cmds, &attrs, |_| true);

    if let UiCommand::Element { attrs: a, .. } = &cmds[0] {
      assert_eq!(a[0].as_str(), Some("typed"), "value attr repatched");
    } else {
      panic!("expected Element");
    }

    // A slot the filter rejects is left untouched.
    zo_state_set_str_slot(430, b"later");
    apply_attr_bindings(&mut cmds, &attrs, |_| false);

    if let UiCommand::Element { attrs: a, .. } = &cmds[0] {
      assert_eq!(a[0].as_str(), Some("typed"), "filtered slot unchanged");
    }
  }

  #[test]
  fn out_of_range_write_does_not_mark_dirty() {
    let _serial = state_lock();
    zo_state_init(8);

    let mut drained = Vec::new();
    drain_dirty(&mut drained);
    drained.clear();

    // Way past the initialized range — write is dropped, so
    // no slot is marked.
    zo_state_set(900_001, 1);

    drain_dirty(&mut drained);

    assert!(!drained.contains(&900_001), "dropped write stays clean");
  }

  /// Test helper: write `bytes` into string slot `slot` via the
  /// public FFI, building the length-prefixed buffer the ABI
  /// expects.
  fn zo_state_set_str_slot(slot: u32, bytes: &[u8]) {
    let payload = encode_length_prefixed(bytes);

    unsafe { zo_state_set_str(slot, payload.as_ptr()) };
  }

  // --- dispatcher index assignment ---
  //
  // We can't easily test the dispatcher itself (cross-thread
  // fn-ptr call without a real runtime), but we CAN verify
  // the index-assignment logic that has to mirror codegen's
  // dedupe scheme. If these diverge the dispatcher routes
  // events to wrong handlers.

  unsafe extern "C" fn noop_dispatch(
    _idx: u32,
    _kind: u32,
    _value_ptr: *const u8,
  ) {
  }

  fn registered_handlers(cmds: &[UiCommand]) -> Vec<String> {
    let registry = build_registry(
      SendPtr(noop_dispatch as _),
      Arc::new(Mutex::new(vec![])),
      RegistryInputs {
        base: cmds.to_vec(),
        lists: Vec::new(),
        attrs: Vec::new(),
        bindings_ptr: SendPtr(std::ptr::null()),
        bindings_count: 0,
        report: Arc::new(Mutex::new(UpdateReport::default())),
      },
    );
    let mut out: Vec<String> = cmds
      .iter()
      .filter_map(|c| match c {
        UiCommand::Event { handler, .. } => {
          registry.has(handler).then_some(handler.clone())
        }
        _ => None,
      })
      .collect();

    out.sort();
    out.dedup();
    out
  }

  #[test]
  fn build_registry_dedupes_handlers() {
    // Same handler on two widgets = one registry entry.
    let cmds = vec![
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
      UiCommand::Event {
        widget_id: "2".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
    ];

    let names = registered_handlers(&cmds);

    assert_eq!(names, vec!["__closure_0".to_string()]);
  }

  #[test]
  fn build_registry_distinct_handlers() {
    let cmds = vec![
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
      UiCommand::Event {
        widget_id: "2".into(),
        event_kind: EventKind::Click,
        handler: "__closure_1".into(),
      },
    ];

    let names = registered_handlers(&cmds);

    assert_eq!(
      names,
      vec!["__closure_0".to_string(), "__closure_1".to_string()]
    );
  }
}
