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
    let mut header_data = Vec::new();
    let mut command_data = Vec::new();
    let mut cmd_specific_data = Vec::new();
    let mut string_table = Vec::new();
    let mut string_offsets = HashMap::default();

    let mut add_string = |s: &str| -> u32 {
      if let Some(&offset) = string_offsets.get(s) {
        return offset;
      }

      let offset = string_table.len() as u32;

      string_offsets.insert(s.to_string(), offset);
      string_table.extend_from_slice(s.as_bytes());
      string_table.push(0);

      offset
    };

    header_data.extend_from_slice(&(commands.len() as u32).to_le_bytes());
    header_data.extend_from_slice(&0u32.to_le_bytes());

    let cmd_data_base =
      TEMPLATE_HEADER_SIZE + (TEMPLATE_CMD_SIZE * commands.len());
    let mut cmd_data_offset = 0usize;

    for cmd in commands {
      let cmd_type = match cmd {
        UiCommand::BeginContainer { .. } => 0u32,
        UiCommand::EndContainer => 1u32,
        UiCommand::Text { .. } => 2u32,
        UiCommand::Button { .. } => 3u32,
        UiCommand::TextInput { .. } => 4u32,
        UiCommand::Image { .. } => 5u32,
        UiCommand::Event { .. } => 6u32,
        UiCommand::StyleSheet { .. } => 7u32,
      };

      command_data.extend_from_slice(&cmd_type.to_le_bytes());
      command_data.extend_from_slice(&0u32.to_le_bytes());

      match cmd {
        UiCommand::BeginContainer { id, direction } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;

          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          let str_offset = add_string(id);

          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
          cmd_specific_data
            .extend_from_slice(&direction.as_u32().to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
          cmd_data_offset += TEMPLATE_CMD_SIZE;
        }
        UiCommand::EndContainer => {
          command_data.extend_from_slice(&0u64.to_le_bytes());
        }
        UiCommand::Text { content, style } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;

          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          let str_offset = add_string(content);

          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
          cmd_specific_data.extend_from_slice(&style.as_u32().to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
          cmd_data_offset += TEMPLATE_CMD_SIZE;
        }
        UiCommand::Button { id, content } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;

          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          cmd_specific_data.extend_from_slice(&id.to_le_bytes());

          let str_offset = add_string(content);

          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u64.to_le_bytes());
          cmd_data_offset += TEMPLATE_CMD_SIZE;
        }
        UiCommand::TextInput {
          id,
          placeholder,
          value,
        } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;

          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          cmd_specific_data.extend_from_slice(&id.to_le_bytes());

          let po = add_string(placeholder);

          cmd_specific_data.extend_from_slice(&po.to_le_bytes());

          let vo = add_string(value);

          cmd_specific_data.extend_from_slice(&vo.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
          cmd_data_offset += TEMPLATE_CMD_SIZE;
        }
        UiCommand::Image {
          id,
          src,
          width,
          height,
        } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;

          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          let io = add_string(id);

          cmd_specific_data.extend_from_slice(&io.to_le_bytes());

          let so = add_string(src);

          cmd_specific_data.extend_from_slice(&so.to_le_bytes());
          cmd_specific_data.extend_from_slice(&width.to_le_bytes());
          cmd_specific_data.extend_from_slice(&height.to_le_bytes());
          cmd_data_offset += TEMPLATE_CMD_SIZE;
        }
        UiCommand::Event { .. } => {
          command_data.extend_from_slice(&0u64.to_le_bytes());
        }
        UiCommand::StyleSheet { .. } => {
          // StyleSheet is handled at a higher level; no
          // per-command binary data needed here.
          command_data.extend_from_slice(&0u64.to_le_bytes());
        }
      }
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
