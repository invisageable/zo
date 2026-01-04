use zo_buffer::Buffer;
use zo_codegen_backend::Artifact;
use zo_emitter_arm::{ARM64Emitter, SP, X0, X1, X2, X16};
use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, Sir};
use zo_ui_protocol::UiCommand;
use zo_value::ValueId;
use zo_writer_macho::{DebugFrameEntry, MachO};

use rustc_hash::FxHashMap as HashMap;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

// // Helper trait extensions for UI protocol types
// trait DirectionExt {
//   fn as_u32(&self) -> u32;
// }
// impl DirectionExt for ContainerDirection {
//   fn as_u32(&self) -> u32 {
//     match self {
//       ContainerDirection::Horizontal => 0,
//       ContainerDirection::Vertical => 1,
//     }
//   }
// }

// trait StyleExt {
//   fn as_u32(&self) -> u32;
// }
// impl StyleExt for TextStyle {
//   fn as_u32(&self) -> u32 {
//     match self {
//       TextStyle::Normal => 0,
//       TextStyle::Heading1 => 1,
//       TextStyle::Heading2 => 2,
//       TextStyle::Heading3 => 3,
//       TextStyle::Paragraph => 4,
//     }
//   }
// }

/// Represents the [`ARM64Gen`] code generation instance.
pub struct ARM64Gen<'a> {
  /// The [`ARM64Emitter`].
  emitter: ARM64Emitter,
  /// String interner for resolving symbols
  interner: &'a Interner,
  /// Function labels (name -> code offset)
  functions: HashMap<Symbol, u32>,
  /// String data to emit at end
  string_data: Vec<(Symbol, Vec<u8>)>,
  /// Current function context
  current_function: Option<Symbol>,
  /// Fixups for string references (position in code -> symbol)
  string_fixups: Vec<(u32, Symbol)>,
  /// Template data sections (symbol -> data)
  template_data: Vec<(Symbol, Vec<u8>)>,
  /// Whether we have templates that need the entry point
  pub has_templates: bool,
}
impl<'a> ARM64Gen<'a> {
  /// Creates a new [`ARM64Gen`] instance.
  pub fn new(interner: &'a Interner) -> Self {
    Self {
      emitter: ARM64Emitter::new(),
      interner,
      functions: HashMap::default(),
      string_data: Vec::new(),
      current_function: None,
      string_fixups: Vec::new(),
      template_data: Vec::new(),
      has_templates: false,
    }
  }

  /// Generates `ARM64` code from SIR.
  pub fn generate(&mut self, sir: &Sir) -> Artifact {
    for insn in &sir.instructions {
      self.translate_insn(insn);
    }

    // Generate _zo_ui_entry_point if we have templates
    if self.has_templates {
      self.generate_ui_entry_point();
    }

    let mut code = self.emitter.code();
    let mut string_offsets = HashMap::default();
    let mut template_offsets = HashMap::default();
    let mut current_offset = code.len();

    // Calculate offsets for string data
    for (symbol, bytes) in &self.string_data {
      string_offsets.insert(*symbol, current_offset);
      current_offset += bytes.len();
    }

    // Calculate offsets for template data
    for (symbol, bytes) in &self.template_data {
      template_offsets.insert(*symbol, current_offset);
      current_offset += bytes.len();
    }

    // Apply fixups - patch ADR instructions with correct offsets
    for (fixup_pos, symbol) in &self.string_fixups {
      let target_offset = string_offsets
        .get(symbol)
        .or_else(|| template_offsets.get(symbol));

      if let Some(offset) = target_offset {
        // Calculate offset from ADR instruction to target
        let offset = (*offset as i32) - (*fixup_pos as i32);

        // Build ADR instruction with correct offset
        let reg = X1;
        let immlo = (offset & 0x3) as u32;
        let immhi = ((offset >> 2) & 0x7FFFF) as u32;

        let insn =
          0x10000000 | (immlo << 29) | (immhi << 5) | (reg.index() as u32);

        // Patch the instruction
        let pos = *fixup_pos as usize;
        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    // Append string data
    for (_symbol, bytes) in &self.string_data {
      code.extend_from_slice(bytes);
    }

    // Append template data
    for (_symbol, bytes) in &self.template_data {
      code.extend_from_slice(bytes);
    }

    Artifact { code }
  }

  /// Generates Mach-O executable from [`Artifact`].
  pub fn generate_macho(&mut self, artifact: Artifact) -> Vec<u8> {
    let mut macho = MachO::new();

    macho.add_code(artifact.code);
    macho.add_data(Vec::new());

    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();

    // Add main symbol if it exists
    if self.interner.symbol("main").is_some() {
      macho.add_function_symbol("_main", 1, 0x100000400, false);
    }

    // Add _zo_ui_entry_point symbol if we have templates
    if self.has_templates {
      let entry_symbol = Symbol(0xFFFF); // Same fixed symbol ID
      if let Some(&offset) = self.functions.get(&entry_symbol) {
        macho.add_function_symbol(
          "_zo_ui_entry_point",
          1,
          0x100000400u64 + offset as u64,
          true,
        );
      }
    }

    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(0x400); // Entry point at start of __text section
    macho.add_dyld_info();
    macho.finish_with_signature()
  }

  /// Generate a complete executable from SIR
  pub fn generate_executable(&mut self, sir: &Sir) -> Vec<u8> {
    let artifact = self.generate(sir);

    self.generate_macho(artifact)
  }

  /// Generates ARM64 assembly text from SIR for display
  pub fn generate_asm(&mut self, sir: &Sir) -> String {
    let mut asm = String::new();
    asm.push_str("  .section __TEXT,__text,regular,pure_instructions\n");
    asm.push_str("  .build_version macos, 11, 0\n");
    asm.push_str("  .globl _main\n");
    asm.push_str("  .p2align 2\n");

    for insn in &sir.instructions {
      self.translate_insn_to_text(insn, &mut asm);
    }

    asm
  }

  /// Translate a single SIR instruction to assembly text
  fn translate_insn_to_text(&mut self, insn: &Insn, asm: &mut String) {
    match insn {
      Insn::FunDef { name, .. } => {
        let func_name = self.interner.get(*name);
        asm.push_str(&format!("\n_{}:\n", func_name));
        self.current_function = Some(*name);
      }

      Insn::ConstInt { value, .. } => {
        if *value <= 65535 {
          asm.push_str(&format!("  mov x0, #{}\n", value));
        } else {
          asm.push_str(&format!("  mov x0, #{}\n", value & 0xFFFF));
          if (*value >> 16) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #16\n",
              (*value >> 16) & 0xFFFF
            ));
          }
          if (*value >> 32) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #32\n",
              (*value >> 32) & 0xFFFF
            ));
          }
          if (*value >> 48) & 0xFFFF != 0 {
            asm.push_str(&format!(
              "  movk x0, #{}, lsl #48\n",
              (*value >> 48) & 0xFFFF
            ));
          }
        }
      }

      Insn::ConstString { symbol, .. } => {
        let string = self.interner.get(*symbol);
        asm.push_str(&format!("  adr x1, .L_str_{}\n", symbol.as_u32()));
        asm.push_str(&format!("  mov x2, #{}\n", string.len()));
      }

      Insn::Call { name, .. } => {
        let func_name = self.interner.get(*name);
        match func_name {
          "show" | "showln" => {
            asm.push_str("  mov x16, #4  ; write syscall\n");
            asm.push_str("  mov x0, #1   ; stdout\n");
            asm.push_str("  svc #0\n");
          }
          _ => {
            asm.push_str(&format!("  bl _{}\n", func_name));
          }
        }
      }

      Insn::Return { .. } => {
        asm.push_str("  mov x0, #0\n");
        asm.push_str("  ret\n");
      }

      Insn::BinOp { op, .. } => match op {
        BinOp::Add => asm.push_str("  add x0, x0, x1\n"),
        BinOp::Sub => asm.push_str("  sub x0, x0, x1\n"),
        BinOp::Mul => asm.push_str("  mul x0, x0, x1\n"),
        BinOp::Div => asm.push_str("  sdiv x0, x0, x1\n"),
        _ => asm.push_str(&format!("  ; TODO: {:?}\n", op)),
      },

      _ => {
        asm.push_str(&format!("  ; TODO: {:?}\n", insn));
      }
    }
  }

  /// Translate a single SIR instruction to ARM64
  fn translate_insn(&mut self, insn: &Insn) {
    match insn {
      Insn::FunDef { name, .. } => {
        let offset = self.emitter.current_offset();

        self.functions.insert(*name, offset);

        self.current_function = Some(*name);

        // Function prologue would go here
        // For now, we'll keep it simple
      }

      Insn::ConstInt { value, .. } => {
        // Constants just get loaded into X0 for now
        // In a real implementation, we'd track which value this creates
        let reg = X0;
        // MOV reg, #value (for small values)
        if *value <= 65535 {
          self.emitter.emit_mov_imm(reg, *value as u16);
        } else {
          // For larger values, use multiple MOV instructions
          self.emitter.emit_mov_imm(reg, (*value & 0xFFFF) as u16);

          if (*value >> 16) & 0xFFFF != 0 {
            self
              .emitter
              .emit_movk(reg, ((*value >> 16) & 0xFFFF) as u16, 16);
          }

          if (*value >> 32) & 0xFFFF != 0 {
            self
              .emitter
              .emit_movk(reg, ((*value >> 32) & 0xFFFF) as u16, 32);
          }

          if (*value >> 48) & 0xFFFF != 0 {
            self
              .emitter
              .emit_movk(reg, ((*value >> 48) & 0xFFFF) as u16, 48);
          }
        }
      }

      Insn::ConstString { symbol, .. } => {
        let mut buffer = Buffer::new();

        let string = self.interner.get(*symbol);

        buffer.bytes(string.as_bytes());
        buffer.bytes(b"\0");

        // Store string data for later emission
        self.string_data.push((*symbol, buffer.finish()));

        // Record fixup position and emit placeholder ADR
        let fixup_pos = self.emitter.current_offset();
        self.string_fixups.push((fixup_pos, *symbol));

        self.emitter.emit_adr(X1, 0);
        self.emitter.emit_mov_imm(X2, string.len() as u16);
      }

      Insn::Call { name, .. } => {
        match self.interner.get(*name) {
          "show" => {
            // System call for write(1, string_ptr, string_len)
            self.emitter.emit_mov_imm(X16, 4); // write syscall
            self.emitter.emit_mov_imm(X0, 1); // stdout
            self.emitter.emit_svc(0);
          }
          "showln" => {
            // First write the string (X1=ptr, X2=len already set)
            self.emitter.emit_mov_imm(X16, 4); // write syscall
            self.emitter.emit_mov_imm(X0, 1); // stdout
            self.emitter.emit_svc(0);

            // Write newline using stack
            self.emitter.emit_mov_imm(X1, 10); // '\n' = 10 in ASCII
            self.emitter.emit_sub_imm(X2, SP, 16); // Use stack pointer - 16 for safety
            self.emitter.emit_strb(X1, X2, 0); // Store byte at stack location
            self.emitter.emit_mov_reg(X1, X2); // X1 = pointer to newline on stack
            self.emitter.emit_mov_imm(X2, 1); // Length = 1
            self.emitter.emit_mov_imm(X16, 4); // write syscall
            self.emitter.emit_mov_imm(X0, 1); // stdout
            self.emitter.emit_svc(0);
          }
          _ => {
            // Check if it's a user-defined function
            if let Some(&func_offset) = self.functions.get(name) {
              // Calculate relative offset for BL instruction
              let current = self.emitter.current_offset();
              let offset = func_offset - current;

              self.emitter.emit_bl(offset as i32);
            }
            // Otherwise, it's an unknown external function
          }
        }
      }

      Insn::Return { value, .. } => {
        // For all functions (including main), just return normally
        // The return value should already be in X0 from the previous
        // instruction
        if value.is_none() {
          // No return value specified, use 0
          self.emitter.emit_mov_imm(X0, 0);
        }

        // Use normal function return for all functions
        self.emitter.emit_ret();
      }

      Insn::BinOp { op, .. } => {
        // For now, assume operands are in X0 and X1
        // Real implementation would track values properly
        let lhs_reg = X0;
        let rhs_reg = X1;
        let dst_reg = X0;

        match op {
          BinOp::Add => {
            self.emitter.emit_add(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::Sub => {
            self.emitter.emit_sub(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::Mul => {
            self.emitter.emit_mul(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::Div => {
            // Use signed division for now
            self.emitter.emit_sdiv(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::Rem => {
            // Modulo: dst = lhs - (lhs / rhs) * rhs
            // Need a temp register for division result
            let temp = X0; // Use X0 as temp

            self.emitter.emit_sdiv(temp, lhs_reg, rhs_reg);
            self.emitter.emit_mul(temp, temp, rhs_reg);
            self.emitter.emit_sub(dst_reg, lhs_reg, temp);
          }
          BinOp::BitAnd => {
            self.emitter.emit_and(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::BitOr => {
            self.emitter.emit_orr(dst_reg, lhs_reg, rhs_reg);
          }
          BinOp::Shl => {
            self.emitter.emit_lsl(dst_reg, lhs_reg, 1);
          }
          BinOp::Shr => {
            self.emitter.emit_lsr(dst_reg, lhs_reg, 1);
          }
          BinOp::Lt => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0xB);
          }
          BinOp::Lte => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0xD);
          }
          BinOp::Gt => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0xC);
          }
          BinOp::Gte => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0xA);
          }
          BinOp::Eq => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0x0);
          }
          BinOp::Neq => {
            self.emitter.emit_cmp(lhs_reg, rhs_reg);
            self.emitter.emit_mov_imm(dst_reg, 1);
            self.emitter.emit_mov_imm(X0, 0);
            self.emitter.emit_csel(dst_reg, dst_reg, X0, 0x1);
          }
          _ => {
            // Other operations not yet implemented
          }
        }
      }

      Insn::VarDef { .. } => {
        // Variable definitions handled in execution phase
      }

      Insn::Template {
        id, name, commands, ..
      } => {
        self.handle_template(*id, *name, commands);
      }

      Insn::Directive { name, value, .. } => {
        let n = self.interner.get(*name);
        if n == "dom" {
          self.emit_render_call(*value);
        }
      }

      _ => {
        // Other instructions not yet implemented
      }
    }
  }

  /// Handle template compilation to static data
  fn handle_template(
    &mut self,
    id: ValueId,
    _name: Option<Symbol>,
    commands: &[UiCommand],
  ) {
    // Generate static command table matching runtime's expected layout:
    // Header: [u32 count][u32 padding]
    // Commands: Each is 16 bytes [u32 type][u32 padding][u64 data_ptr]
    // Command data structures follow
    // String table at the end

    let mut header_data = Vec::new();
    let mut command_data = Vec::new();
    let mut cmd_specific_data = Vec::new();
    let mut string_table = Vec::new();
    let mut string_offsets = HashMap::default();

    // Helper to add string to table and return offset
    let mut add_string = |s: &str| -> u32 {
      if let Some(&offset) = string_offsets.get(s) {
        return offset;
      }
      let offset = string_table.len() as u32;
      string_offsets.insert(s.to_string(), offset);
      string_table.extend_from_slice(s.as_bytes());
      string_table.push(0); // null terminate
      offset
    };

    // Write header: count + padding
    header_data.extend_from_slice(&(commands.len() as u32).to_le_bytes());
    header_data.extend_from_slice(&0u32.to_le_bytes()); // padding for 8-byte alignment

    // Calculate base offset for command-specific data structures
    // Header (8) + Commands (16 * count)
    let cmd_data_base = 8 + (16 * commands.len());
    let mut cmd_data_offset = 0usize;

    // Write each command (exactly 16 bytes each)
    for cmd in commands {
      // Command type (4 bytes)
      let cmd_type = match cmd {
        UiCommand::BeginContainer { .. } => 0u32,
        UiCommand::EndContainer => 1u32,
        UiCommand::Text { .. } => 2u32,
        UiCommand::Button { .. } => 3u32,
        UiCommand::TextInput { .. } => 4u32,
        UiCommand::Image { .. } => 5u32,
        UiCommand::Event { .. } => 6u32,
      };
      command_data.extend_from_slice(&cmd_type.to_le_bytes());
      command_data.extend_from_slice(&0u32.to_le_bytes()); // padding

      // Data pointer (8 bytes) - will need relocation
      // For now, store offset that will be converted to pointer
      match cmd {
        UiCommand::BeginContainer { id, direction } => {
          // Store offset to command-specific data
          let data_ptr_offset = cmd_data_base + cmd_data_offset;
          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          // Build command-specific data structure
          let str_offset = add_string(id);
          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes()); // padding
          cmd_specific_data
            .extend_from_slice(&direction.as_u32().to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes()); // padding
          cmd_data_offset += 16; // Each data structure is 16 bytes
        }
        UiCommand::EndContainer => {
          command_data.extend_from_slice(&0u64.to_le_bytes()); // null pointer
        }
        UiCommand::Text { content, style } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;
          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          let str_offset = add_string(content);
          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes()); // padding
          cmd_specific_data.extend_from_slice(&style.as_u32().to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes()); // padding
          cmd_data_offset += 16;
        }
        UiCommand::Button { id, content } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;
          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());

          cmd_specific_data.extend_from_slice(&id.to_le_bytes());
          let str_offset = add_string(content);
          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u64.to_le_bytes()); // padding
          cmd_data_offset += 16;
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
          let placeholder_offset = add_string(placeholder);
          cmd_specific_data
            .extend_from_slice(&placeholder_offset.to_le_bytes());
          let value_offset = add_string(value);
          cmd_specific_data.extend_from_slice(&value_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes()); // padding
          cmd_data_offset += 16;
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

          let id_offset = add_string(id);
          cmd_specific_data.extend_from_slice(&id_offset.to_le_bytes());
          let src_offset = add_string(src);
          cmd_specific_data.extend_from_slice(&src_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&width.to_le_bytes());
          cmd_specific_data.extend_from_slice(&height.to_le_bytes());
          cmd_data_offset += 16;
        }
        UiCommand::Event { .. } => {
          command_data.extend_from_slice(&0u64.to_le_bytes()); // null pointer for now
        }
      }
    }

    // Combine all data sections
    let mut final_data = Vec::new();
    final_data.extend_from_slice(&header_data);
    final_data.extend_from_slice(&command_data);
    final_data.extend_from_slice(&cmd_specific_data);
    final_data.extend_from_slice(&string_table);

    // Store template data with a generated symbol
    let template_symbol = Symbol(id.0 + 0x1000); // Offset to avoid collisions
    self.template_data.push((template_symbol, final_data));
    self.has_templates = true;
  }

  /// Generate the _zo_ui_entry_point function that returns template data
  fn generate_ui_entry_point(&mut self) {
    // Function prologue
    let entry_symbol = Symbol(0xFFFF); // Use a fixed symbol ID for _zo_ui_entry_point
    self
      .functions
      .insert(entry_symbol, self.emitter.current_offset());

    // Return pointer to first template data
    // For now, return the first template if it exists
    if let Some((symbol, _)) = self.template_data.first() {
      let fixup_pos = self.emitter.current_offset();
      self.string_fixups.push((fixup_pos, *symbol));
      self.emitter.emit_adr(X0, 0); // Will be fixed up to point to template data
    } else {
      // Return null if no templates
      self.emitter.emit_mov_imm(X0, 0);
    }

    // Return
    self.emitter.emit_ret();
  }

  /// Emit a call to the runtime render function
  fn emit_render_call(&mut self, value: zo_value::ValueId) {
    // Load template data pointer into X0
    // For now, use a placeholder approach
    // In a real implementation, we'd look up the template data location

    // ADR X0, template_data
    let template_symbol = Symbol(value.0 + 0x1000);
    let fixup_pos = self.emitter.current_offset();
    self.string_fixups.push((fixup_pos, template_symbol));
    self.emitter.emit_adr(X0, 0); // Placeholder offset

    // Call runtime render function
    // BL _zo_render_template
    // For now, we'll emit a placeholder system call
    self.emitter.emit_mov_imm(X16, 4); // write syscall as placeholder
    self.emitter.emit_mov_imm(X0, 1); // stdout
    self.emitter.emit_svc(0);
  }

  /// Write binary to file and make it executable
  pub fn write_executable(
    binary: Vec<u8>,
    path: impl AsRef<Path>,
  ) -> std::io::Result<()> {
    fs::write(&path, binary)?;

    let metadata = fs::metadata(&path)?;
    let mut permissions = metadata.permissions();

    permissions.set_mode(0o755); // make executable (chmod +x).

    fs::set_permissions(path, permissions)?;

    Ok(())
  }

  /// Generate a complete "Hello, World" executable
  /// This is a minimal implementation for testing
  pub fn generate_hello_world() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();

    // "Hello, World!\n" string will be in __DATA segment
    let hello_str = b"Hello, World!\n";

    // Generate ARM64 code for main function
    // On macOS ARM64:
    // - Syscall number goes in X16 register
    // - Arguments in X0-X7
    // - Use svc #0 to make the call
    // - write = 4, exit = 1

    // write(1, "Hello, World!\n", 14)
    emitter.emit_mov_imm(X16, 4); // syscall 4 = write
    emitter.emit_mov_imm(X0, 1); // fd = stdout

    // Calculate offset to string data that will be placed right after this code
    // PC at ADR instruction: 0x408
    // String location: 0x420
    // Offset = 0x420 - 0x408 = 0x18 = 24 bytes
    let string_offset_from_adr = 0x18;

    // Use a simple forward reference to data placed right after the code
    emitter.emit_adr(X1, string_offset_from_adr);

    emitter.emit_mov_imm(X2, 14); // length = 14 bytes
    emitter.emit_svc(0); // Make system call with #0

    // exit(0)
    emitter.emit_mov_imm(X16, 1); // syscall 1 = exit
    emitter.emit_mov_imm(X0, 0); // exit code = 0
    emitter.emit_svc(0); // Make system call with #0

    // Get the code and append the string data directly after it
    let mut code = emitter.code();
    code.extend_from_slice(hello_str);

    // Create Mach-O executable
    let mut macho = MachO::new();

    // Add the combined code+data
    macho.add_code(code);

    // No separate data segment needed - data is embedded in code
    macho.add_data(Vec::new());

    // Build the Mach-O file structure
    // IMPORTANT: Segments must be added first!
    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();

    // Add _main symbol at the entry point (section 1 = __TEXT,__text)
    macho.add_function_symbol("_main", 1, 0x100000400, false);

    // Then add other load commands
    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(0x400); // Entry point at start of __text section
    macho.add_dyld_info();
    macho.finish()
  }

  /// Generate a complete "Hello, World" executable with code signature
  pub fn generate_hello_world_signed() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();

    // "Hello, World!\n" string will be in __DATA segment
    let hello_str = b"Hello, World!\n";

    // Generate ARM64 code for main function (same as before)
    emitter.emit_mov_imm(X16, 4); // syscall 4 = write
    emitter.emit_mov_imm(X0, 1); // fd = stdout

    let string_offset_from_adr = 0x18;
    emitter.emit_adr(X1, string_offset_from_adr);

    emitter.emit_mov_imm(X2, 14); // length = 14 bytes
    emitter.emit_svc(0); // Make system call

    // exit(0)
    emitter.emit_mov_imm(X16, 1); // syscall 1 = exit
    emitter.emit_mov_imm(X0, 0); // exit code = 0
    emitter.emit_svc(0); // Make system call

    // Get the code and append the string data
    let mut code = emitter.code();

    code.extend_from_slice(hello_str);

    let code_len = code.len();

    let mut macho = MachO::new();

    macho.add_code(code);
    macho.add_data(Vec::new());

    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();

    // Add _main symbol at the entry point (section 1 = __TEXT,__text)
    macho.add_function_symbol("_main", 1, 0x100000400, false);

    // Add comprehensive debug symbols
    macho.add_source_file_info("hello_world.zo", "/tmp/zo");
    macho.add_compiler_info("zo v0.1.0", 2); // Optimization level 2
    macho.add_function_brackets("_main", 1, 0x100000400, code_len as u64);
    macho.add_source_line(1, 0x100000400); // Line 1 at function start

    let mut frame_entry = DebugFrameEntry::new(0x100000400, code_len as u64);

    frame_entry.add_def_cfa(31, 0); // Define CFA as SP+0
    frame_entry.add_nop(); // Padding
    macho.add_debug_frame_entry(frame_entry);

    // Add load commands
    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(0x400); // Entry point at start of __text section
    macho.add_dyld_info();
    macho.finish_with_signature()
  }
}
