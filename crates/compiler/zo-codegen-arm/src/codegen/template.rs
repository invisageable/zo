//! Template data compilation for reactive UI.
//!
//! Translates `Insn::Template` into a postcard-encoded byte
//! blob and a per-template list of event-handler symbols.
//! `emit_render_call` later builds a `ZoRuntimeContext` on
//! the stack and calls `_zo_run_native`. After every user
//! function is emitted, `generate_template_dispatchers`
//! synthesises one `_zo_dispatch_<id>(handler_idx, kind)`
//! per template that routes `widget_handler_index` →
//! handler closure via a `cmp / b.ne / adr / br` chain.

use super::{
  ARM64Gen, TEMPLATE_DISPATCHER_SYMBOL_OFFSET, TEMPLATE_SYMBOL_OFFSET,
  UI_ENTRY_SYMBOL,
};

use zo_emitter_arm::{X0, X1, X2, X9};
use zo_interner::Symbol;
use zo_ui_protocol::UiCommand;
use zo_ui_protocol::codec;
use zo_value::ValueId;

impl<'a> ARM64Gen<'a> {
  /// Encode the template's command stream via postcard and
  /// stash it under `Symbol(id + TEMPLATE_SYMBOL_OFFSET)`.
  /// The bytes land in the output's rodata region alongside
  /// string literals; the symbol resolves to its file/load
  /// address via the `string_fixups` machinery, which
  /// already considers `template_offsets` when patching
  /// ADRs.
  ///
  /// Also records the template's event-handler symbol list
  /// in declaration order so `generate_template_dispatchers`
  /// can emit the matching `_zo_dispatch_<id>` switch.
  pub(super) fn handle_template(
    &mut self,
    id: ValueId,
    _name: Option<Symbol>,
    commands: &[UiCommand],
  ) {
    // postcard::Error here means the workspace-derived
    // Serialize impl panicked or hit an internal limit —
    // both unrecoverable. Embed an empty payload so the
    // symbol still exists; the runtime surfaces the decode
    // failure cleanly via `_zo_run_native`'s eprintln.
    let bytes = codec::encode(commands).unwrap_or_default();
    let template_symbol = Symbol(id.0 + TEMPLATE_SYMBOL_OFFSET);

    // Collect UNIQUE event-handler names in first-seen
    // order. Two `@click={f}` attributes on different
    // widgets share the same handler function and must
    // map to the same dispatcher index. The runtime
    // mirror walks the same decoded command stream so its
    // dedupe produces an identical list.
    //
    // The position of a handler in this list IS the
    // `widget_handler_index` the runtime passes back into
    // `_zo_dispatch_<id>(idx, kind)`. Empty list → no
    // dispatcher emitted, `ctx.handle_event` stays null.
    //
    // The interner is held immutably here so we can't
    // intern fresh symbols — store the strings and let
    // `generate_template_dispatchers` resolve them
    // against `self.functions` later, whose keys were
    // interned by the executor at FunDef emission.
    let mut seen: std::collections::HashSet<String> =
      std::collections::HashSet::new();
    let handlers: Vec<String> = commands
      .iter()
      .filter_map(|cmd| match cmd {
        UiCommand::Event { handler, .. } => {
          if seen.insert(handler.clone()) {
            Some(handler.clone())
          } else {
            None
          }
        }
        _ => None,
      })
      .collect();

    self.template_data.push((template_symbol, bytes));
    self.template_handlers.insert(id, handlers);
    self.has_templates = true;
  }

  /// Emit one `_zo_dispatch_<id>` function per template
  /// that has at least one event handler. Body shape:
  ///
  /// ```text
  /// mov  x1, x2          ; param[1] for closure = &Event
  /// cmp  w0, #0
  /// b.ne .next0
  /// adr  x9, handler_0   ; function_addr_fixup
  /// br   x9              ; tail-call → handler returns to
  ///                      ; runtime, skipping dispatcher
  /// .next0: cmp w0, #1
  /// b.ne .next1
  /// adr  x9, handler_1
  /// br   x9
  /// ...
  /// .nextN: ret          ; unknown idx, no-op
  /// ```
  ///
  /// The `mov x1, x2` is the payload-forwarding hop. The
  /// runtime invokes us as
  /// `_zo_dispatch_<id>(widget_idx, kind, event_ptr)` —
  /// `event_ptr` lands in x2 per AAPCS. The user closure's
  /// `e: Event` parameter expects an Event-struct base
  /// pointer in x1 (the closure's `param[1]` after the
  /// captures `param[0]`); copying x2 → x1 satisfies it
  /// without us needing a frame. The runtime side
  /// allocates the 8-byte Event struct (a single
  /// length-prefixed-string pointer) on its own stack so
  /// the address stays valid for the closure body.
  ///
  /// `br x9` stays a tail call — the handler returns
  /// straight to the runtime, no dispatcher epilogue
  /// needed. `function_addr_fixups` resolves each `ADR`
  /// once all callee functions are laid out in
  /// `self.functions`.
  ///
  /// Must run AFTER every user function (including
  /// closure handlers) has been emitted — otherwise the
  /// fixups won't find the callee offsets.
  pub(super) fn generate_template_dispatchers(&mut self) {
    // Take to satisfy the borrow checker — we mutate
    // `self.emitter` etc. while walking the table.
    let handlers = std::mem::take(&mut self.template_handlers);

    // O(F) name → Symbol index built once instead of per
    // handler. Without this the inner `find` over
    // `self.functions.keys()` re-walked every function
    // for every event handler — quadratic for templates
    // with many handlers in a program with many fns.
    let sym_by_name: std::collections::HashMap<String, zo_interner::Symbol> =
      self
        .functions
        .keys()
        .copied()
        .map(|(name, _pack)| (self.interner.get(name).to_string(), name))
        .collect();

    // Per-block layout (from `cmp_imm`):
    //   PC+0  : cmp_imm        (4 bytes)
    //   PC+4  : b.ne <skip>    (4 bytes)
    //   PC+8  : adr x9, <h>    (4 bytes)
    //   PC+12 : br  x9         (4 bytes)
    //   PC+16 : next block
    //
    // `b.ne` is PC-relative against its OWN address — to
    // land at the next block we need offset 12 (skip the
    // adr+br pair AND the bne itself's slot). Passing 8
    // would land ON the `br x9` with an uninitialised
    // X9 and segfault.
    const SKIP_BYTES: i32 = 12;

    for (id, names) in &handlers {
      if names.is_empty() {
        continue;
      }

      let dispatcher_symbol = Symbol(id.0 + TEMPLATE_DISPATCHER_SYMBOL_OFFSET);

      self
        .functions
        .insert((dispatcher_symbol, None), self.emitter.current_offset());

      // One-time payload-forwarding: x1 (currently
      // holding `event_kind`, which the dispatcher and
      // user closures both ignore) gets overwritten with
      // x2 (`event_ptr`) so the upcoming `br x9` lands
      // in the closure with `param[1] = &Event` already
      // set. Zero-cost when the program has no payload-
      // bearing handlers — x2 is null in that case and
      // the closure never reads param[1].
      self.emitter.emit_mov_reg(X1, X2);

      for (i, handler_name) in names.iter().enumerate() {
        let Some(&handler_sym) = sym_by_name.get(handler_name) else {
          // Handler missing — should never happen for a
          // well-formed program (every Event command's
          // handler was a closure that produced a
          // FunDef). Skip the block so the dispatcher
          // stays well-formed; this widget will simply
          // not fire.
          continue;
        };

        // `cmp w0/x0, #i` — runtime passes the
        // widget_handler_index in x0. emit_cmp_imm uses
        // the 64-bit form; upper bits are zero
        // (handler_idx is u32) so the wider compare is
        // equivalent.
        self.emitter.emit_cmp_imm(X0, i as u16);

        // `b.ne +8` → jump past the adr+br pair to the
        // next block.
        self.emitter.emit_bne(SKIP_BYTES);

        // `adr x9, handler_i` — resolved post-emission
        // by `function_addr_fixups`.
        let adr_pos = self.emitter.current_offset();

        self.emitter.emit_adr(X9, 0);
        self.function_addr_fixups.push((adr_pos, (handler_sym, None)));

        // `br x9` — tail call into the handler.
        self.emitter.emit_br(X9);
      }

      // Trailing `ret` — unknown widget index falls
      // through to here and returns to the runtime
      // without invoking any handler.
      self.emitter.emit_ret();
    }

    // Restore the table so any downstream observer
    // (debug dumps, future passes) still sees it.
    self.template_handlers = handlers;
  }

  /// Generate the `_zo_ui_entry_point` function — returns
  /// a pointer to the first template's encoded bytes. Kept
  /// for the dlopen-style host loader path
  /// (`zo_ui_protocol::loader::LibraryLoader`); the
  /// `_zo_run_native` direct-call path bypasses it.
  pub(super) fn generate_ui_entry_point(&mut self) {
    let entry_symbol = Symbol(UI_ENTRY_SYMBOL);

    self
      .functions
      .insert((entry_symbol, None), self.emitter.current_offset());

    if let Some((symbol, _)) = self.template_data.first() {
      let fixup_pos = self.emitter.current_offset();

      self.string_fixups.push((fixup_pos, *symbol));
      self.emitter.emit_adr(X0, 0);
    } else {
      self.emitter.emit_mov_imm(X0, 0);
    }

    self.emitter.emit_ret();
  }
}
