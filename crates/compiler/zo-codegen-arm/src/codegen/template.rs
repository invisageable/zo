//! Template data compilation for reactive UI.
//!
//! Translates `Insn::Template` and `Insn::Directive { name:
//! "dom" }` into static binary data that the runtime loader
//! parses back into `Vec<UiCommand>`.

use super::{
  ARM64Gen, TEMPLATE_CMD_SIZE, TEMPLATE_HEADER_SIZE, TEMPLATE_SYMBOL_OFFSET,
  UI_ENTRY_SYMBOL,
};

use zo_emitter_arm::X0;
use zo_interner::Symbol;
use zo_ui_protocol::UiCommand;
use zo_value::ValueId;

use rustc_hash::FxHashMap as HashMap;

impl<'a> ARM64Gen<'a> {
  /// Handle template compilation to static data.
  pub(super) fn handle_template(
    &mut self,
    id: ValueId,
    _name: Option<Symbol>,
    commands: &[UiCommand],
  ) {
    // Binary encoder for the unified Element model is a work
    // in progress. For R1 we emit a header + a placeholder type
    // code + null data pointer for every command so the dylib
    // layout stays well-formed; the interactive `zo run` path
    // bypasses this entirely and renders from the in-memory
    // command buffer. The full encoder lands in a follow-up
    // alongside a matching `zo-ui-protocol/src/loader.rs`
    // decoder rewrite.
    let _ = (TEMPLATE_CMD_SIZE, TEMPLATE_HEADER_SIZE);

    let mut header_data = Vec::new();
    let mut command_data = Vec::new();
    let cmd_specific_data: Vec<u8> = Vec::new();
    let string_table: Vec<u8> = Vec::new();
    let _ = HashMap::<String, u32>::default();

    header_data.extend_from_slice(&(commands.len() as u32).to_le_bytes());
    header_data.extend_from_slice(&0u32.to_le_bytes());

    for cmd in commands {
      command_data.extend_from_slice(&cmd.type_code().to_le_bytes());
      command_data.extend_from_slice(&0u32.to_le_bytes());
      command_data.extend_from_slice(&0u64.to_le_bytes());
    }

    let mut final_data = Vec::new();

    final_data.extend_from_slice(&header_data);
    final_data.extend_from_slice(&command_data);
    final_data.extend_from_slice(&cmd_specific_data);
    final_data.extend_from_slice(&string_table);

    let template_symbol = Symbol(id.0 + TEMPLATE_SYMBOL_OFFSET);

    self.template_data.push((template_symbol, final_data));
    self.has_templates = true;
  }

  /// Generate the `_zo_ui_entry_point` function.
  pub(super) fn generate_ui_entry_point(&mut self) {
    let entry_symbol = Symbol(UI_ENTRY_SYMBOL);

    self
      .functions
      .insert(entry_symbol, self.emitter.current_offset());

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
