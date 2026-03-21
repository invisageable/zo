use zo_buffer::Buffer;
use zo_codegen_backend::Artifact;
use zo_emitter_arm::{
  ARM64Emitter, D0, D1, FpRegister, Register, SP, X0, X1, X2, X16, X29, X30,
};
use zo_interner::{Interner, Symbol};
use zo_register_allocation::{RegAlloc, SpillKind};
use zo_sir::{BinOp, Insn, Sir};
use zo_ui_protocol::UiCommand;
use zo_value::ValueId;
use zo_writer_macho::{DebugFrameEntry, MachO};

use rustc_hash::FxHashMap as HashMap;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Represents the [`ARM64Gen`] code generation instance.
pub struct ARM64Gen<'a> {
  /// The [`ARM64Emitter`].
  emitter: ARM64Emitter,
  /// String interner for resolving symbols.
  interner: &'a Interner,
  /// Function labels (name -> code offset).
  functions: HashMap<Symbol, u32>,
  /// String data to emit at end.
  string_data: Vec<(Symbol, Vec<u8>)>,
  /// Current function context.
  current_function: Option<Symbol>,
  /// Fixups for string references (position in code -> symbol).
  string_fixups: Vec<(u32, Symbol)>,
  /// Template data sections (symbol -> data).
  template_data: Vec<(Symbol, Vec<u8>)>,
  /// Whether we have templates that need the entry point.
  pub has_templates: bool,
  /// The label offsets: label_id → byte offset in code.
  labels: HashMap<u32, u32>,
  /// The branch fixups: (code_offset, target_label_id).
  branch_fixups: Vec<(u32, u32)>,
  /// Register allocation result.
  reg_alloc: Option<RegAlloc>,
  /// Current function's start index into SIR instructions.
  current_fn_start: Option<usize>,
  /// Mutable variable stack slots: name → offset from SP.
  mutable_slots: HashMap<u32, u32>,
  /// Next mutable variable slot.
  next_mut_slot: u32,
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
      labels: HashMap::default(),
      branch_fixups: Vec::new(),
      reg_alloc: None,
      current_fn_start: None,
      mutable_slots: HashMap::default(),
      next_mut_slot: 0,
    }
  }

  // --- Register allocation helpers ---

  /// Look up the allocated register for a ValueId.
  fn alloc_reg(&self, vid: ValueId) -> Option<Register> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get(vid))
      .map(Register::new)
  }

  /// Look up the allocated GP register for the value
  /// defined at instruction `idx`.
  fn reg_for_insn(&self, idx: usize) -> Option<Register> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.value_id_at(idx))
      .and_then(|vid| self.alloc_reg(vid))
  }

  /// Look up the allocated FP register for a ValueId.
  fn alloc_fp_reg(&self, vid: ValueId) -> Option<FpRegister> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.get_fp(vid))
      .map(FpRegister::new)
  }

  /// Look up the allocated FP register for the value
  /// defined at instruction `idx`.
  fn fp_reg_for_insn(&self, idx: usize) -> Option<FpRegister> {
    self
      .reg_alloc
      .as_ref()
      .and_then(|a| a.value_id_at(idx))
      .and_then(|vid| self.alloc_fp_reg(vid))
  }

  /// Check if a ValueId was produced by a ConstString.
  fn is_string_value(&self, vid: ValueId, all_insns: &[Insn]) -> bool {
    self
      .find_producing_insn(vid, all_insns)
      .is_some_and(|insn| matches!(insn, Insn::ConstString { .. }))
  }

  /// Check if a ValueId was produced by a float instruction.
  fn is_float_value(&self, vid: ValueId, all_insns: &[Insn]) -> bool {
    self
      .find_producing_insn(vid, all_insns)
      .is_some_and(|insn| match insn {
        Insn::ConstFloat { .. } => true,
        Insn::BinOp { ty_id, .. }
        | Insn::Load { ty_id, .. }
        | Insn::Call { ty_id, .. } => ty_id.0 >= 15 && ty_id.0 <= 17,
        _ => false,
      })
  }

  /// Find the SIR instruction that produced a ValueId.
  fn find_producing_insn<'b>(
    &self,
    vid: ValueId,
    all_insns: &'b [Insn],
  ) -> Option<&'b Insn> {
    self.reg_alloc.as_ref().and_then(|a| {
      a.value_ids
        .iter()
        .enumerate()
        .find(|(_, v)| **v == Some(vid))
        .and_then(|(i, _)| all_insns.get(i))
    })
  }

  /// Emit a single spill operation (GP or FP).
  fn emit_spill_op(&mut self, kind: &SpillKind) {
    match kind {
      SpillKind::Store {
        reg,
        slot,
        is_fp: false,
      } => {
        self
          .emitter
          .emit_str(Register::new(*reg), SP, (*slot * 8) as i16);
      }
      SpillKind::Load {
        reg,
        slot,
        is_fp: false,
      } => {
        self
          .emitter
          .emit_ldr(Register::new(*reg), SP, (*slot * 8) as i16);
      }
      SpillKind::Store {
        reg,
        slot,
        is_fp: true,
      } => {
        self
          .emitter
          .emit_str_fp(FpRegister::new(*reg), SP, (*slot * 8) as u16);
      }
      SpillKind::Load {
        reg,
        slot,
        is_fp: true,
      } => {
        self
          .emitter
          .emit_ldr_fp(FpRegister::new(*reg), SP, (*slot * 8) as u16);
      }
    }
  }

  /// Emit spill ops before instruction `idx`.
  fn emit_spills_before(&mut self, idx: usize) {
    let ops = self
      .reg_alloc
      .as_ref()
      .map(|a| {
        a.spill_ops
          .iter()
          .filter(|op| op.insn_idx == idx && op.before)
          .map(|op| op.kind.clone())
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();

    for kind in &ops {
      self.emit_spill_op(kind);
    }
  }

  /// Emit spill ops after instruction `idx`.
  fn emit_spills_after(&mut self, idx: usize) {
    let ops = self
      .reg_alloc
      .as_ref()
      .map(|a| {
        a.spill_ops
          .iter()
          .filter(|op| op.insn_idx == idx && !op.before)
          .map(|op| op.kind.clone())
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();

    for kind in &ops {
      self.emit_spill_op(kind);
    }
  }

  /// Load a 64-bit immediate into a register using
  /// MOV + MOVK sequence.
  fn emit_mov_imm_64(&mut self, reg: Register, value: u64) {
    if value <= 65535 {
      self.emitter.emit_mov_imm(reg, value as u16);
    } else {
      self.emitter.emit_mov_imm(reg, (value & 0xFFFF) as u16);
      if (value >> 16) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 16) & 0xFFFF) as u16, 16);
      }
      if (value >> 32) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 32) & 0xFFFF) as u16, 32);
      }
      if (value >> 48) & 0xFFFF != 0 {
        self
          .emitter
          .emit_movk(reg, ((value >> 48) & 0xFFFF) as u16, 48);
      }
    }
  }

  // --- Code generation ---

  /// Generates `ARM64` code from SIR.
  pub fn generate(&mut self, sir: &Sir) -> Artifact {
    // Run register allocation before codegen.
    self.reg_alloc =
      Some(RegAlloc::allocate(&sir.instructions, sir.next_value_id));

    let insns = &sir.instructions;

    for (idx, insn) in insns.iter().enumerate() {
      self.emit_spills_before(idx);
      self.translate_insn(insn, idx, insns);
      self.emit_spills_after(idx);
    }

    // Generate _zo_ui_entry_point if we have templates.
    if self.has_templates {
      self.generate_ui_entry_point();
    }

    let mut code = self.emitter.code();
    let mut string_offsets = HashMap::default();
    let mut template_offsets = HashMap::default();
    let mut current_offset = code.len();

    for (symbol, bytes) in &self.string_data {
      string_offsets.insert(*symbol, current_offset);
      current_offset += bytes.len();
    }

    for (symbol, bytes) in &self.template_data {
      template_offsets.insert(*symbol, current_offset);
      current_offset += bytes.len();
    }

    // Apply string fixups.
    for (fixup_pos, symbol) in &self.string_fixups {
      let target_offset = string_offsets
        .get(symbol)
        .or_else(|| template_offsets.get(symbol));

      if let Some(offset) = target_offset {
        let offset = (*offset as i32) - (*fixup_pos as i32);
        let reg = X1;
        let immlo = (offset & 0x3) as u32;
        let immhi = ((offset >> 2) & 0x7FFFF) as u32;
        let insn =
          0x10000000 | (immlo << 29) | (immhi << 5) | (reg.index() as u32);
        let pos = *fixup_pos as usize;
        code[pos..pos + 4].copy_from_slice(&insn.to_le_bytes());
      }
    }

    // Apply branch fixups.
    for (fixup_pos, target_label) in &self.branch_fixups {
      if let Some(&label_offset) = self.labels.get(target_label) {
        let relative = (label_offset as i32 - *fixup_pos as i32) >> 2;
        let pos = *fixup_pos as usize;
        let existing =
          u32::from_le_bytes(code[pos..pos + 4].try_into().unwrap());

        let patched = if existing & 0xFC000000 == 0x14000000 {
          0x14000000 | ((relative as u32) & 0x3FFFFFF)
        } else if existing & 0x7F000000 == 0x34000000 {
          let sf_and_op = existing & 0xFF000000;
          let rt = existing & 0x1F;
          sf_and_op | (((relative as u32) & 0x7FFFF) << 5) | rt
        } else {
          existing
        };

        code[pos..pos + 4].copy_from_slice(&patched.to_le_bytes());
      }
    }

    for (_symbol, bytes) in &self.string_data {
      code.extend_from_slice(bytes);
    }
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

    if let Some(main_sym) = self.interner.symbol("main") {
      let offset = self.functions.get(&main_sym).copied().unwrap_or(0);
      macho.add_function_symbol(
        "_main",
        1,
        0x100000400u64 + offset as u64,
        false,
      );
    }

    if self.has_templates {
      let entry_symbol = Symbol(0xFFFF);
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

    // Entry point must point to the actual main function,
    // not always 0x400 (which is only correct when main
    // is the first function in the code section).
    let main_entry = self
      .interner
      .symbol("main")
      .and_then(|s| self.functions.get(&s).copied())
      .map(|off| 0x400u64 + off as u64)
      .unwrap_or(0x400);
    macho.add_main(main_entry);

    macho.add_dyld_info();
    macho.finish_with_signature()
  }

  /// Generate a complete executable from SIR.
  pub fn generate_executable(&mut self, sir: &Sir) -> Vec<u8> {
    let artifact = self.generate(sir);
    self.generate_macho(artifact)
  }

  /// Generates ARM64 assembly text from SIR for display.
  pub fn generate_asm(&mut self, sir: &Sir) -> String {
    let mut asm = String::new();
    asm.push_str(
      "  .section __TEXT,__text,\
       regular,pure_instructions\n",
    );
    asm.push_str("  .build_version macos, 11, 0\n");
    asm.push_str("  .globl _main\n");
    asm.push_str("  .p2align 2\n");

    for insn in &sir.instructions {
      self.translate_insn_to_text(insn, &mut asm);
    }

    asm
  }

  /// Translate a single SIR instruction to assembly text.
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
          "eshow" | "eshowln" => {
            asm.push_str("  mov x16, #4  ; write syscall\n");
            asm.push_str("  mov x0, #2   ; stderr\n");
            asm.push_str("  svc #0\n");
          }
          "flush" => {
            asm.push_str("  ; flush (no-op)\n");
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

  /// Translate a single SIR instruction to ARM64.
  fn translate_insn(&mut self, insn: &Insn, idx: usize, all_insns: &[Insn]) {
    match insn {
      Insn::FunDef { name, .. } => {
        let offset = self.emitter.current_offset();

        self.functions.insert(*name, offset);
        self.current_function = Some(*name);
        self.current_fn_start = Some(idx);
        self.mutable_slots.clear();
        self.next_mut_slot = 0;

        // Function prologue: save FP/LR if non-leaf.
        if let Some(info) = self
          .reg_alloc
          .as_ref()
          .and_then(|a| a.function_info.get(&idx))
        {
          if info.has_calls {
            self.emitter.emit_stp(X29, X30, SP, -16);
          }

          // Reserve space for spills + mutable slots.
          // Add 64 bytes (8 slots) for mutable vars.
          let frame = (info.spill_size + 64 + 15) & !15;

          if frame > 0 {
            self.emitter.emit_sub_imm(SP, SP, frame as u16);
          }
        }
      }

      Insn::ConstInt { value, .. } => {
        if let Some(reg) = self.reg_for_insn(idx) {
          self.emit_mov_imm_64(reg, *value);
        }
      }

      Insn::ConstFloat { value, .. } => {
        // Load f64 bits into GP scratch, FMOV to FP.
        let fp_dst = self.fp_reg_for_insn(idx).unwrap_or(D0);
        let bits = value.to_bits();

        self.emit_mov_imm_64(X16, bits);
        self.emitter.emit_fmov_gp_to_fp(fp_dst, X16);
      }

      Insn::ConstBool { value, .. } => {
        if let Some(reg) = self.reg_for_insn(idx) {
          self.emitter.emit_mov_imm(reg, *value as u16);
        }
      }

      Insn::ConstString { symbol, .. } => {
        let mut buffer = Buffer::new();
        let string = self.interner.get(*symbol);

        buffer.bytes(string.as_bytes());
        buffer.bytes(b"\0");

        self.string_data.push((*symbol, buffer.finish()));

        let fixup_pos = self.emitter.current_offset();

        self.string_fixups.push((fixup_pos, *symbol));
        self.emitter.emit_adr(X1, 0);
        self.emitter.emit_mov_imm(X2, string.len() as u16);
      }

      Insn::Load { dst, src, .. } => {
        if *src >= 100 {
          // Mutable variable: LDR from stack slot.
          let slot = *src - 100;

          if let Some(dst_reg) = self.alloc_reg(*dst)
            && let Some(&offset) = self.mutable_slots.get(&slot)
          {
            self.emitter.emit_ldr(dst_reg, SP, offset as i16);
          }
        } else if let Some(fp_dst) = self.alloc_fp_reg(*dst) {
          // Float parameter: arrives in D[src].
          let fp_src = FpRegister::new(*src as u8);

          if fp_dst != fp_src {
            self.emitter.emit_fmov_fp(fp_dst, fp_src);
          }
        } else if let Some(dst_reg) = self.alloc_reg(*dst) {
          // GP parameter: arrives in X[src].
          let src_reg = Register::new(*src as u8);

          if dst_reg != src_reg {
            self.emitter.emit_mov_reg(dst_reg, src_reg);
          }
        }
      }

      Insn::BinOp {
        dst,
        op,
        lhs,
        rhs,
        ty_id,
      } => {
        let is_float = ty_id.0 >= 15 && ty_id.0 <= 17;

        if is_float {
          let fl = self.alloc_fp_reg(*lhs).unwrap_or(D0);
          let fr = self.alloc_fp_reg(*rhs).unwrap_or(D1);
          let fd = self.alloc_fp_reg(*dst).unwrap_or(D0);
          match op {
            BinOp::Add => self.emitter.emit_fadd(fd, fl, fr),
            BinOp::Sub => self.emitter.emit_fsub(fd, fl, fr),
            BinOp::Mul => self.emitter.emit_fmul(fd, fl, fr),
            BinOp::Div => self.emitter.emit_fdiv(fd, fl, fr),
            BinOp::Lt
            | BinOp::Lte
            | BinOp::Gt
            | BinOp::Gte
            | BinOp::Eq
            | BinOp::Neq => {
              self.emitter.emit_fcmp(fl, fr);
            }
            _ => {}
          }
        } else {
          // Integer: use allocated registers.
          let d = self.alloc_reg(*dst).unwrap_or(X0);
          let l = self.alloc_reg(*lhs).unwrap_or(X0);
          let r = self.alloc_reg(*rhs).unwrap_or(X1);

          match op {
            BinOp::Add => {
              self.emitter.emit_add(d, l, r);
            }
            BinOp::Sub => {
              self.emitter.emit_sub(d, l, r);
            }
            BinOp::Mul => {
              self.emitter.emit_mul(d, l, r);
            }
            BinOp::Div => {
              self.emitter.emit_sdiv(d, l, r);
            }
            BinOp::Rem => {
              // dst = lhs - (lhs / rhs) * rhs
              // Use X16 as scratch.
              self.emitter.emit_sdiv(X16, l, r);
              self.emitter.emit_mul(X16, X16, r);
              self.emitter.emit_sub(d, l, X16);
            }
            BinOp::BitAnd => {
              self.emitter.emit_and(d, l, r);
            }
            BinOp::BitOr => {
              self.emitter.emit_orr(d, l, r);
            }
            BinOp::Shl => {
              self.emitter.emit_lsl(d, l, 1);
            }
            BinOp::Shr => {
              self.emitter.emit_lsr(d, l, 1);
            }
            BinOp::Lt => {
              self.emit_cmp_csel(d, l, r, 0xB);
            }
            BinOp::Lte => {
              self.emit_cmp_csel(d, l, r, 0xD);
            }
            BinOp::Gt => {
              self.emit_cmp_csel(d, l, r, 0xC);
            }
            BinOp::Gte => {
              self.emit_cmp_csel(d, l, r, 0xA);
            }
            BinOp::Eq => {
              self.emit_cmp_csel(d, l, r, 0x0);
            }
            BinOp::Neq => {
              self.emit_cmp_csel(d, l, r, 0x1);
            }
            _ => {}
          }
        }
      }

      Insn::Call { name, args, .. } => {
        match self.interner.get(*name) {
          "show" => {
            self.emitter.emit_mov_imm(X16, 4);
            self.emitter.emit_mov_imm(X0, 1);
            self.emitter.emit_svc(0);
          }
          "showln" => {
            // Compile-time type dispatch (Graydon style).
            let arg_vid = if args.is_empty() { None } else { Some(args[0]) };

            let is_str =
              arg_vid.is_some_and(|v| self.is_string_value(v, all_insns));

            let is_flt =
              arg_vid.is_some_and(|v| self.is_float_value(v, all_insns));

            if is_flt {
              // Float: convert to int part + "." + frac,
              // then write. Move float arg to D0 first.
              if let Some(fp_src) = arg_vid.and_then(|v| self.alloc_fp_reg(v))
                && fp_src != D0
              {
                self.emitter.emit_fmov_fp(D0, fp_src);
              }

              self.emit_ftoa_and_write(1);
            } else if !is_str && arg_vid.is_some() {
              // Int: move to X0, itoa, write.
              if let Some(src) = arg_vid.and_then(|v| self.alloc_reg(v))
                && src != X0
              {
                self.emitter.emit_mov_reg(X0, src);
              }

              self.emit_itoa_and_write(1);
            } else {
              // String: X1=ptr, X2=len already set.
              self.emitter.emit_mov_imm(X16, 4);
              self.emitter.emit_mov_imm(X0, 1);
              self.emitter.emit_svc(0);
            }

            // Write newline.
            self.emitter.emit_mov_imm(X1, 10);
            self.emitter.emit_sub_imm(X2, SP, 16);
            self.emitter.emit_strb(X1, X2, 0);
            self.emitter.emit_mov_reg(X1, X2);
            self.emitter.emit_mov_imm(X2, 1);
            self.emitter.emit_mov_imm(X16, 4);
            self.emitter.emit_mov_imm(X0, 1);
            self.emitter.emit_svc(0);
          }
          "eshow" => {
            self.emitter.emit_mov_imm(X16, 4);
            self.emitter.emit_mov_imm(X0, 2);
            self.emitter.emit_svc(0);
          }
          "eshowln" => {
            self.emitter.emit_mov_imm(X16, 4);
            self.emitter.emit_mov_imm(X0, 2);
            self.emitter.emit_svc(0);

            self.emitter.emit_mov_imm(X1, 10);
            self.emitter.emit_sub_imm(X2, SP, 16);
            self.emitter.emit_strb(X1, X2, 0);
            self.emitter.emit_mov_reg(X1, X2);
            self.emitter.emit_mov_imm(X2, 1);
            self.emitter.emit_mov_imm(X16, 4);
            self.emitter.emit_mov_imm(X0, 2);
            self.emitter.emit_svc(0);
          }
          "flush" => {}
          _ => {
            // Move args to X0-X7 (GP) or D0-D7 (FP).
            for (i, arg) in args.iter().enumerate() {
              if i >= 8 {
                break;
              }

              if let Some(fp_src) = self.alloc_fp_reg(*arg) {
                let fp_dst = FpRegister::new(i as u8);

                if fp_src != fp_dst {
                  self.emitter.emit_fmov_fp(fp_dst, fp_src);
                }
              } else if let Some(src_reg) = self.alloc_reg(*arg) {
                let dst_reg = Register::new(i as u8);

                if src_reg != dst_reg {
                  self.emitter.emit_mov_reg(dst_reg, src_reg);
                }
              }
            }

            // BL to user-defined function.
            if let Some(&func_offset) = self.functions.get(name) {
              let current = self.emitter.current_offset();
              let offset = func_offset as i32 - current as i32;
              self.emitter.emit_bl(offset);
            }

            // Move result to allocated register.
            // Float results arrive in D0, GP in X0.
            if let Some(fp_result) = self.fp_reg_for_insn(idx) {
              if fp_result != D0 {
                self.emitter.emit_fmov_fp(fp_result, D0);
              }
            } else if let Some(result_reg) = self.reg_for_insn(idx)
              && result_reg != X0
            {
              self.emitter.emit_mov_reg(result_reg, X0);
            }
          }
        }
      }

      Insn::Return { value, .. } => {
        // Move return value to X0.
        if let Some(vid) = value {
          if let Some(src_reg) = self.alloc_reg(*vid)
            && src_reg != X0
          {
            self.emitter.emit_mov_reg(X0, src_reg);
          }
        } else {
          self.emitter.emit_mov_imm(X0, 0);
        }

        // Function epilogue.
        if let Some(start) = self.current_fn_start
          && let Some(info) = self
            .reg_alloc
            .as_ref()
            .and_then(|a| a.function_info.get(&start))
        {
          let frame = (info.spill_size + 64 + 15) & !15;
          if frame > 0 {
            self.emitter.emit_add_imm(SP, SP, frame as u16);
          }
          if info.has_calls {
            self.emitter.emit_ldp(X29, X30, SP, 16);
          }
        }

        self.emitter.emit_ret();
      }

      Insn::VarDef { .. } => {
        // Handled in execution phase.
      }

      Insn::Store { name, value, .. } => {
        // Mutable variable write: STR value to stack slot.
        // Allocate slot on first Store, reuse after.
        let slot_key = name.as_u32();
        let offset = if let Some(&off) = self.mutable_slots.get(&slot_key) {
          off
        } else {
          // Allocate after spill slots. Use the
          // function's spill_size as base offset.
          let base = self
            .current_fn_start
            .and_then(|s| {
              self
                .reg_alloc
                .as_ref()
                .and_then(|a| a.function_info.get(&s))
            })
            .map(|info| info.spill_size)
            .unwrap_or(0);

          let off = base + self.next_mut_slot * 8;

          self.mutable_slots.insert(slot_key, off);

          self.next_mut_slot += 1;

          off
        };
        if let Some(src_reg) = self.alloc_reg(*value) {
          self.emitter.emit_str(src_reg, SP, offset as i16);
        }
      }

      Insn::Template {
        id,
        name: tpl_name,
        commands,
        ..
      } => {
        self.handle_template(*id, *tpl_name, commands);
      }

      Insn::Directive { name, value, .. } => {
        let n = self.interner.get(*name);
        if n == "dom" {
          self.emit_render_call(*value);
        }
      }

      Insn::Label { id } => {
        self.labels.insert(*id, self.emitter.current_offset());
      }

      Insn::Jump { target } => {
        self
          .branch_fixups
          .push((self.emitter.current_offset(), *target));
        self.emitter.emit_b(0);
      }

      Insn::BranchIfNot { cond, target } => {
        let reg = self.alloc_reg(*cond).unwrap_or(X0);
        self
          .branch_fixups
          .push((self.emitter.current_offset(), *target));
        self.emitter.emit_cbz(reg, 0);
      }

      Insn::ArrayLiteral { elements, .. } => {
        // Layout on stack: [len, e0, e1, ..., eN]
        // Allocate (1 + N) * 8 bytes below current SP.
        let n = elements.len() as u16;
        let size = (n + 1) * 8;
        let aligned = (size + 15) & !15;

        self.emitter.emit_sub_imm(SP, SP, aligned);

        // Store length at [SP + 0].
        self.emitter.emit_mov_imm(X16, n);
        self.emitter.emit_str(X16, SP, 0);

        // Store each element at [SP + (i+1)*8].
        for (i, elem) in elements.iter().enumerate() {
          if let Some(reg) = self.alloc_reg(*elem) {
            self.emitter.emit_str(reg, SP, ((i + 1) * 8) as i16);
          }
        }

        // Result: pointer to array (SP).
        if let Some(dst) = self.reg_for_insn(idx) {
          self.emitter.emit_mov_reg(dst, SP);
        }
      }

      Insn::ArrayIndex {
        dst, array, index, ..
      } => {
        // Load element at base + 8 + index * 8.
        // Use X16 as scratch.
        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let arr_reg = self.alloc_reg(*array).unwrap_or(X0);
          let idx_reg = self.alloc_reg(*index).unwrap_or(X1);

          // X16 = index << 3 (index * 8)
          self.emitter.emit_lsl(X16, idx_reg, 3);
          // X16 = array_base + X16
          self.emitter.emit_add(X16, arr_reg, X16);
          // X16 = X16 + 8 (skip length field)
          self.emitter.emit_add_imm(X16, X16, 8);
          // dst = [X16]
          self.emitter.emit_ldr(dst_reg, X16, 0);
        }
      }

      Insn::ArrayLen { dst, array, .. } => {
        // Length at [base + 0].
        if let Some(dst_reg) = self.alloc_reg(*dst) {
          let arr_reg = self.alloc_reg(*array).unwrap_or(X0);

          self.emitter.emit_ldr(dst_reg, arr_reg, 0);
        }
      }

      _ => {}
    }
  }

  /// Convert X0 (unsigned int) to decimal string on the
  /// stack and write it to file descriptor `fd`.
  ///
  /// Algorithm: repeatedly divide by 10, push ASCII digits
  /// onto a stack buffer in reverse, then write.
  /// Convert D0 (double) to decimal string and write to fd.
  ///
  /// Strategy: print integer part, ".", then 6 fractional
  /// digits. Handles negative by printing "-" prefix.
  fn emit_ftoa_and_write(&mut self, fd: u16) {
    // FCVTZS X0, D0 — integer part (truncated).
    self.emitter.emit_fcvtzs(X0, D0);

    // Print the integer part via itoa.
    self.emit_itoa_and_write(fd);

    // Print "."
    self.emitter.emit_sub_imm(SP, SP, 16);
    self.emitter.emit_mov_imm(X1, b'.' as u16);
    self.emitter.emit_strb(X1, SP, 0);
    // ADD X1, SP, #0 (can't use MOV for SP — XZR alias).
    self.emitter.emit_add_imm(X1, SP, 0);
    self.emitter.emit_mov_imm(X2, 1);
    self.emitter.emit_mov_imm(X16, 4);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);
    self.emitter.emit_add_imm(SP, SP, 16);

    // Compute fractional part: frac = (D0 - int_part) * 1e6.
    // Reload D0's integer part into X0, convert back to D1.
    self.emitter.emit_fcvtzs(X0, D0);
    self.emitter.emit_scvtf(D1, X0);
    // D0 = D0 - D1 (fractional part, 0.0 to 0.999...)
    self.emitter.emit_fsub(D0, D0, D1);

    // Multiply by 1000000 to get 6 decimal digits.
    // Load 1000000.0 into D1 via GP.
    let million_bits = 1_000_000.0f64.to_bits();

    self
      .emitter
      .emit_mov_imm(X0, (million_bits & 0xFFFF) as u16);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 16) & 0xFFFF) as u16, 16);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 32) & 0xFFFF) as u16, 32);
    self
      .emitter
      .emit_movk(X0, ((million_bits >> 48) & 0xFFFF) as u16, 48);

    self.emitter.emit_fmov_gp_to_fp(D1, X0);

    // D0 = frac * 1000000.0
    self.emitter.emit_fmul(D0, D0, D1);
    // X0 = int(D0)
    self.emitter.emit_fcvtzs(X0, D0);

    // Print 6 digits via itoa.
    self.emit_itoa_and_write(fd);
  }

  fn emit_itoa_and_write(&mut self, fd: u16) {
    // Reserve 32-byte buffer on stack.
    self.emitter.emit_sub_imm(SP, SP, 32);

    // X1 = end of buffer (write pointer, works backward)
    self.emitter.emit_add_imm(X1, SP, 31);
    // X2 = 0 (length counter)
    self.emitter.emit_mov_imm(X2, 0);
    // X3 = 10 (divisor)
    let x3 = Register::new(3);

    self.emitter.emit_mov_imm(x3, 10);

    // Handle zero case: if X0 == 0, write "0".
    let loop_start = self.emitter.current_offset();

    // X4 = X0 / 10
    let x4 = Register::new(4);
    let x5 = Register::new(5);

    self.emitter.emit_udiv(x4, X0, x3);
    // X5 = X0 - X4 * 10 (remainder = digit)
    self.emitter.emit_msub(x5, x4, x3, X0);
    // X5 += '0'
    self.emitter.emit_add_imm(x5, x5, 48);
    // Store byte at [X1], X1 -= 1
    self.emitter.emit_strb_post_dec(x5, X1);
    // X2 += 1 (length)
    self.emitter.emit_add_imm(X2, X2, 1);
    // X0 = quotient
    self.emitter.emit_mov_reg(X0, x4);
    // If X0 != 0, loop
    let cbnz_offset = loop_start as i32 - self.emitter.current_offset() as i32;

    self.emitter.emit_cbnz(X0, cbnz_offset);

    // X1 points one past the first digit — adjust.
    self.emitter.emit_add_imm(X1, X1, 1);

    // Write syscall: write(fd, X1, X2)
    self.emitter.emit_mov_imm(X16, 4);
    self.emitter.emit_mov_imm(X0, fd);
    self.emitter.emit_svc(0);

    // Restore stack.
    self.emitter.emit_add_imm(SP, SP, 32);
  }

  /// Emit CMP + MOV 1 + MOV 0 + CSEL pattern for
  /// comparisons. Uses X16 as zero scratch to avoid
  /// clobbering dst.
  fn emit_cmp_csel(
    &mut self,
    dst: Register,
    lhs: Register,
    rhs: Register,
    cond: u8,
  ) {
    self.emitter.emit_cmp(lhs, rhs);
    self.emitter.emit_mov_imm(dst, 1);
    self.emitter.emit_mov_imm(X16, 0);
    self.emitter.emit_csel(dst, dst, X16, cond);
  }

  /// Handle template compilation to static data.
  fn handle_template(
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

    let cmd_data_base = 8 + (16 * commands.len());
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
          cmd_data_offset += 16;
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
          cmd_data_offset += 16;
        }
        UiCommand::Button { id, content } => {
          let data_ptr_offset = cmd_data_base + cmd_data_offset;
          command_data
            .extend_from_slice(&(data_ptr_offset as u64).to_le_bytes());
          cmd_specific_data.extend_from_slice(&id.to_le_bytes());
          let str_offset = add_string(content);
          cmd_specific_data.extend_from_slice(&str_offset.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u64.to_le_bytes());
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
          let po = add_string(placeholder);
          cmd_specific_data.extend_from_slice(&po.to_le_bytes());
          let vo = add_string(value);
          cmd_specific_data.extend_from_slice(&vo.to_le_bytes());
          cmd_specific_data.extend_from_slice(&0u32.to_le_bytes());
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
          let io = add_string(id);
          cmd_specific_data.extend_from_slice(&io.to_le_bytes());
          let so = add_string(src);
          cmd_specific_data.extend_from_slice(&so.to_le_bytes());
          cmd_specific_data.extend_from_slice(&width.to_le_bytes());
          cmd_specific_data.extend_from_slice(&height.to_le_bytes());
          cmd_data_offset += 16;
        }
        UiCommand::Event { .. } => {
          command_data.extend_from_slice(&0u64.to_le_bytes());
        }
      }
    }

    let mut final_data = Vec::new();
    final_data.extend_from_slice(&header_data);
    final_data.extend_from_slice(&command_data);
    final_data.extend_from_slice(&cmd_specific_data);
    final_data.extend_from_slice(&string_table);

    let template_symbol = Symbol(id.0 + 0x1000);
    self.template_data.push((template_symbol, final_data));
    self.has_templates = true;
  }

  /// Generate the _zo_ui_entry_point function.
  fn generate_ui_entry_point(&mut self) {
    let entry_symbol = Symbol(0xFFFF);
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

  /// Emit a call to the runtime render function.
  fn emit_render_call(&mut self, value: ValueId) {
    let template_symbol = Symbol(value.0 + 0x1000);
    let fixup_pos = self.emitter.current_offset();
    self.string_fixups.push((fixup_pos, template_symbol));
    self.emitter.emit_adr(X0, 0);

    self.emitter.emit_mov_imm(X16, 4);
    self.emitter.emit_mov_imm(X0, 1);
    self.emitter.emit_svc(0);
  }

  /// Write binary to file and make it executable.
  pub fn write_executable(
    binary: Vec<u8>,
    path: impl AsRef<Path>,
  ) -> std::io::Result<()> {
    fs::write(&path, binary)?;

    #[cfg(unix)]
    {
      let metadata = fs::metadata(&path)?;
      let mut permissions = metadata.permissions();
      permissions.set_mode(0o755);
      fs::set_permissions(&path, permissions)?;
    }

    Ok(())
  }

  /// Generate a complete "Hello, World" executable.
  pub fn generate_hello_world() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();
    let hello_str = b"Hello, World!\n";

    emitter.emit_mov_imm(X16, 4);
    emitter.emit_mov_imm(X0, 1);

    let string_offset_from_adr = 0x18;
    emitter.emit_adr(X1, string_offset_from_adr);
    emitter.emit_mov_imm(X2, 14);
    emitter.emit_svc(0);

    emitter.emit_mov_imm(X16, 1);
    emitter.emit_mov_imm(X0, 0);
    emitter.emit_svc(0);

    let mut code = emitter.code();
    code.extend_from_slice(hello_str);

    let mut macho = MachO::new();
    macho.add_code(code);
    macho.add_data(Vec::new());
    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();
    macho.add_function_symbol("_main", 1, 0x100000400, false);
    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(0x400);
    macho.add_dyld_info();
    macho.finish()
  }

  /// Generate a complete "Hello, World" executable with
  /// code signature.
  pub fn generate_hello_world_signed() -> Vec<u8> {
    let mut emitter = ARM64Emitter::new();
    let hello_str = b"Hello, World!\n";

    emitter.emit_mov_imm(X16, 4);
    emitter.emit_mov_imm(X0, 1);

    let string_offset_from_adr = 0x18;
    emitter.emit_adr(X1, string_offset_from_adr);
    emitter.emit_mov_imm(X2, 14);
    emitter.emit_svc(0);

    emitter.emit_mov_imm(X16, 1);
    emitter.emit_mov_imm(X0, 0);
    emitter.emit_svc(0);

    let mut code = emitter.code();
    code.extend_from_slice(hello_str);
    let code_len = code.len();

    let mut macho = MachO::new();
    macho.add_code(code);
    macho.add_data(Vec::new());
    macho.add_pagezero_segment();
    macho.add_text_segment();
    macho.add_data_segment();
    macho.add_function_symbol("_main", 1, 0x100000400, false);

    macho.add_source_file_info("hello_world.zo", "/tmp/zo");
    macho.add_compiler_info("zo v0.1.0", 2);
    macho.add_function_brackets("_main", 1, 0x100000400, code_len as u64);
    macho.add_source_line(1, 0x100000400);

    let mut frame_entry = DebugFrameEntry::new(0x100000400, code_len as u64);
    frame_entry.add_def_cfa(31, 0);
    frame_entry.add_nop();
    macho.add_debug_frame_entry(frame_entry);

    macho.add_dylinker();
    macho.add_dylib("/usr/lib/libSystem.B.dylib");
    macho.add_uuid();
    macho.add_build_version();
    macho.add_source_version();
    macho.add_main(0x400);
    macho.add_dyld_info();
    macho.finish_with_signature()
  }
}
