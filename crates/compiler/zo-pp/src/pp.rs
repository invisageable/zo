use crate::printee::Printee;

use zo_buffer::Buffer;
use zo_codegen_backend::{Artifact, Target};
use zo_interner::Interner;
use zo_sir::{BinOp, Insn, Sir, UnOp};
use zo_token::{Token, TokenBuffer};
use zo_tree::Tree;

/// Represents a [`PrettyPrinter`] instance.
pub struct PrettyPrinter {
  /// The bytes buffer.
  buffer: Buffer,
}
impl PrettyPrinter {
  /// Creates a new [`PrettyPrinter`] instance.
  pub fn new() -> Self {
    Self {
      buffer: Buffer::new(),
    }
  }

  pub fn finish(self) -> Vec<u8> {
    self.buffer.finish()
  }

  // Token formatting methods
  pub fn token_header(&mut self) {
    self.buffer.str("├─ KIND      LEXEME    SPAN\n");
    self.buffer.str("│\n");
  }

  pub fn token_row(
    &mut self,
    kind: &str,
    lexeme: &str,
    start: u32,
    end: u32,
    is_last: bool,
  ) {
    // Tree branch character
    if is_last {
      self.buffer.str("└─ ");
    } else {
      self.buffer.str("├─ ");
    }

    // Fixed-width columns for alignment
    self.buffer.str(kind);
    self.padding(10 - kind.len().min(10));

    self.buffer.str(lexeme);
    self.padding(10 - lexeme.len().min(10));

    self.buffer.char(b'(');
    self.buffer.u32(start);
    self.buffer.str("..");
    self.buffer.u32(end);
    self.buffer.char(b')');
    self.buffer.newline();
  }

  // Tree formatting methods
  pub fn tree_header(&mut self, node_count: usize) {
    self.buffer.str("├── POSTORDER TREE — ");
    self.buffer.u32(node_count as u32);
    self.buffer.str(" nodes.\n");
    self.buffer.str("│\n");
  }

  pub fn tree_node(
    &mut self,
    index: u32,
    depth: usize,
    content: &str,
    is_last: bool,
    parent_continues: &[bool],
  ) {
    // Add indentation for nested levels
    for i in 0..depth {
      if i == depth - 1 {
        // Last level - show the branch
        if is_last {
          self.buffer.str("    └── ");
        } else {
          self.buffer.str("    ├── ");
        }
      } else {
        // Parent levels - show continuation lines
        if i < parent_continues.len() && parent_continues[i] {
          self.buffer.str("│   ");
        } else {
          self.buffer.str("    ");
        }
      }
    }

    // Root level nodes
    if depth == 0 {
      if is_last {
        self.buffer.str("└── ");
      } else {
        self.buffer.str("├── ");
      }
    }

    self.buffer.u32(index);
    self.buffer.str(". ");
    self.buffer.str(content);
    self.buffer.newline();
  }

  /// Writes the Sir header output.
  pub fn sir_header(&mut self) {
    self.buffer.str("SIR INSTRUCTION STREAM:\n");
    self.buffer.str("───────────────────────\n");
  }

  pub fn sir_function(&mut self, name: &str) {
    self.buffer.char(b'@');
    self.buffer.str(name);
    self.buffer.str(":\n");
  }

  pub fn sir_instruction(&mut self, instr: &str) {
    self.buffer.str("  ");
    self.buffer.str(instr);
    self.buffer.newline();
  }

  pub fn format_sir(&mut self, sir: &Sir, interner: &Interner) {
    self.sir_header();

    // Track function definitions and bodies
    // let mut current_function: Option<String> = None;
    let mut in_function_body = false;

    for (idx, insn) in sir.instructions.iter().enumerate() {
      match insn {
        Insn::FunDef { name, .. } => {
          if in_function_body {
            self.buffer.newline();

            // in_function_body = false;
          }

          let name = interner.get(*name);
          // current_function = Some(name.to_string());

          self.sir_function(name);

          in_function_body = true;
        }
        Insn::Return { value, .. } => {
          if let Some(v) = value {
            let return_value = format!("ret %{v}");

            self.sir_instruction(&return_value);
          } else {
            self.sir_instruction("ret void");
          }
        }
        Insn::ConstInt { value, .. } => {
          let int = format!("%{idx} = const {value} : i32");
          self.sir_instruction(&int);
        }
        Insn::ConstFloat { value, .. } => {
          let int = format!("%{idx} = const {value} : f32");
          self.sir_instruction(&int);
        }
        Insn::ConstBool { value, .. } => {
          let boolean = format!("%{idx} = const {value} : bool");
          self.sir_instruction(&boolean);
        }
        Insn::ConstString { symbol, .. } => {
          let content = interner.get(*symbol);
          let string = format!("%{idx} = const \"{content}\" : str");
          self.sir_instruction(&string);
        }
        Insn::BinOp {
          dst, op, lhs, rhs, ..
        } => {
          let op = match op {
            BinOp::Add => "add",
            BinOp::Sub => "sub",
            BinOp::Mul => "mul",
            BinOp::Div => "div",
            BinOp::Rem => "rem",
            BinOp::Eq => "eq",
            BinOp::Neq => "neq",
            BinOp::Lt => "lt",
            BinOp::Lte => "lte",
            BinOp::Gt => "gt",
            BinOp::Gte => "gte",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::BitAnd => "bitand",
            BinOp::BitOr => "bitor",
            BinOp::BitXor => "bitxor",
            BinOp::Shl => "shl",
            BinOp::Shr => "shr",
          };

          let binop = format!("%{dst} = {op} %{lhs}, %{rhs}");

          self.sir_instruction(&binop);
        }
        Insn::UnOp { op, rhs, .. } => {
          let op = match op {
            UnOp::Neg => "neg",
            UnOp::Not => "not",
            UnOp::BitNot => "bitnot",
            UnOp::Ref => "ref",
            UnOp::Deref => "deref",
          };

          let unop = format!("%{idx} = {op} %{rhs}");

          self.sir_instruction(&unop);
        }
        Insn::VarDef {
          name,
          init,
          mutability,
          ..
        } => {
          let name = interner.get(*name);

          let mutability = match mutability {
            zo_value::Mutability::No => "imu",
            zo_value::Mutability::Yes => "mut",
          };

          if let Some(value) = init {
            let var = format!("{mutability} {name} = %{value}");
            self.sir_instruction(&var);
          } else {
            let var = format!("{mutability} {name} = undef");
            self.sir_instruction(&var);
          }
        }
        Insn::Store { name, value, .. } => {
          let name = interner.get(*name);
          let store = format!("store {name}, %{value}");
          self.sir_instruction(&store);
        }
        Insn::Load { dst, src, .. } => {
          let load = format!("%{dst} = load param[{src}]");
          self.sir_instruction(&load);
        }
        Insn::Call { name, args, .. } => {
          let name = interner.get(*name);
          let args = args.iter().map(|v| format!("%{v}")).collect::<Vec<_>>();
          let call = format!("%{idx} = call {name}({})", args.join(", "));
          self.sir_instruction(&call);
        }
        _ => todo!(),
      }
    }

    // Add newline after last function
    if in_function_body {
      self.buffer.newline();
    }
  }

  // Assembly formatting methods
  pub fn asm_header(&mut self, arch: &str) {
    self.buffer.str(arch);
    self.buffer.str(" ASSEMBLY:\n");
    self.buffer.str("───────────────\n");
  }

  pub fn asm_instruction(&mut self, offset: u32, bytes: &[u8], mnemonic: &str) {
    let offset = format!("{offset:04x}");

    self.buffer.str(&offset);
    self.buffer.str(": ");

    // Hex bytes (fixed width)
    for byte in bytes {
      let hex = format!("{byte:02x}");

      self.buffer.str(&hex);
    }

    self.padding(12 - (bytes.len() * 2).min(12));

    self.buffer.str(mnemonic);
    self.buffer.newline();
  }

  pub fn format_asm(&mut self, artifact: &Artifact, target: Target) {
    // Determine architecture name
    let arch_name = match target {
      Target::Arm64AppleDarwin
      | Target::Arm64PcWindowsMsvc
      | Target::Arm64UnknownLinuxGnu => "ARM64",
      Target::X8664AppleDarwin
      | Target::X8664PcWindowsMsvc
      | Target::X8664UnknownLinuxGnu => "X86-64",
      Target::Wasm32UnknownUnknown => "WASM32",
    };

    self.asm_header(arch_name);

    // Disassemble based on target
    match target {
      Target::Arm64AppleDarwin
      | Target::Arm64PcWindowsMsvc
      | Target::Arm64UnknownLinuxGnu => {
        self.disassemble_arm64(&artifact.code);
      }
      Target::X8664AppleDarwin
      | Target::X8664PcWindowsMsvc
      | Target::X8664UnknownLinuxGnu => {
        todo!("X86-64 disassembly not yet implemented\n");
      }
      Target::Wasm32UnknownUnknown => {
        todo!("WASM disassembly not yet implemented\n");
      }
    }
  }

  fn disassemble_arm64(&mut self, code: &[u8]) {
    let mut offset = 0u32;
    let mut i = 0;

    while i + 3 < code.len() {
      // Read 32-bit instruction in little-endian
      let insn =
        u32::from_le_bytes([code[i], code[i + 1], code[i + 2], code[i + 3]]);

      // Decode the instruction
      let mnemonic = self.decode_arm64_insn(insn);

      // Format and print
      self.asm_instruction(offset, &code[i..i + 4], &mnemonic);

      offset += 4;
      i += 4;
    }
  }

  fn decode_arm64_insn(&self, insn: u32) -> String {
    // Basic ARM64 instruction decoding
    // This is a simplified decoder for common instructions

    // Check for special instructions first
    match insn {
      0xD503237F => return "pacibsp".to_string(),
      0xD50323FF => return "autibsp".to_string(),
      0xD65F03C0 => return "ret".to_string(),
      0xD503201F => return "nop".to_string(),
      _ => {}
    }

    // Extract common fields
    // let op0 = (insn >> 25) & 0xF;
    // let op1 = (insn >> 21) & 0xF;
    let rd = insn & 0x1F;

    // Decode based on instruction class
    if (insn & 0x1F000000) == 0x10000000 {
      // ADR instruction
      let imm = self.decode_adr_imm(insn);

      return format!("adr x{rd}, #{imm}");
    }

    if (insn & 0x9F000000) == 0x90000000 {
      // ADRP instruction
      let imm = self.decode_adr_imm(insn) << 12;

      return format!("adrp x{rd}, #{imm:#x}");
    }

    if (insn & 0xFFE00000) == 0xD2800000 {
      // MOVZ (MOV immediate)
      let imm = (insn >> 5) & 0xFFFF;

      return format!("mov x{rd}, #{imm}");
    }

    if (insn & 0xFFE00000) == 0x52800000 {
      // MOVZ W register
      let imm = (insn >> 5) & 0xFFFF;

      return format!("mov w{rd}, #{imm}");
    }

    if (insn & 0xFFE00000) == 0xF2800000 {
      // MOVK
      let imm = (insn >> 5) & 0xFFFF;
      let shift = ((insn >> 21) & 0x3) * 16;

      return format!("movk x{rd}, #{imm:#x}, lsl #{shift}");
    }

    if (insn & 0xFFE0FFE0) == 0xAA0003E0 {
      // MOV register (ORR with XZR)
      let rm = (insn >> 16) & 0x1F;

      return format!("mov x{rd}, x{rm}");
    }

    if (insn & 0xFF000000) == 0x8B000000 {
      // ADD (extended register)
      let rn = (insn >> 5) & 0x1F;
      let rm = (insn >> 16) & 0x1F;

      return format!("add x{rd}, x{rn}, x{rm}");
    }

    if (insn & 0xFF000000) == 0xCB000000 {
      // SUB (extended register)
      let rn = (insn >> 5) & 0x1F;
      let rm = (insn >> 16) & 0x1F;

      return format!("sub x{rd}, x{rn}, x{rm}");
    }

    if (insn & 0xFFC00000) == 0xA9000000 {
      // STP (Store Pair)
      let rt = insn & 0x1F;
      let rt2 = (insn >> 10) & 0x1F;
      let rn = (insn >> 5) & 0x1F;
      let imm = ((insn >> 15) & 0x7F) as i32;
      let offset = (imm - (if imm & 0x40 != 0 { 0x80 } else { 0 })) * 8;

      if insn & 0x800000 != 0 {
        // Pre-index
        return format!("stp x{rt}, x{rt2}, [x{rn}, #{offset}]!");
      } else {
        return format!("stp x{rt}, x{rt2}, [x{rn}, #{offset}]");
      }
    }

    if (insn & 0xFFC00000) == 0xA8C00000 {
      // LDP (Load Pair)
      let rt = insn & 0x1F;
      let rt2 = (insn >> 10) & 0x1F;
      let rn = (insn >> 5) & 0x1F;
      let imm = ((insn >> 15) & 0x7F) as i32;
      let offset = (imm - (if imm & 0x40 != 0 { 0x80 } else { 0 })) * 8;

      return format!("ldp x{rt}, x{rt2}, [x{rn}], #{offset}");
    }

    if (insn & 0xFF800000) == 0x91000000 {
      // ADD immediate
      let rn = (insn >> 5) & 0x1F;
      let imm = (insn >> 10) & 0xFFF;

      return format!("add x{rd}, x{rn}, #{imm}");
    }

    if (insn & 0xFF800000) == 0xD1000000 {
      // SUB immediate
      let rn = (insn >> 5) & 0x1F;
      let imm = (insn >> 10) & 0xFFF;

      return format!("sub x{rd}, x{rn}, #{imm}");
    }

    if (insn & 0xFC000000) == 0x94000000 {
      // BL (Branch with Link)
      let imm = (insn & 0x3FFFFFF) as i32;

      let offset =
        (imm - (if imm & 0x2000000 != 0 { 0x4000000 } else { 0 })) * 4;

      return format!("bl #{offset:#x}");
    }

    // Default: show raw hex
    format!("0x{insn:08x}")
  }

  fn decode_adr_imm(&self, insn: u32) -> i32 {
    let immlo = ((insn >> 29) & 0x3) as i32;
    let immhi = ((insn >> 5) & 0x7FFFF) as i32;
    let imm = (immhi << 2) | immlo;

    // Sign extend if necessary
    if imm & 0x100000 != 0 {
      imm | 0xFFE00000u32 as i32
    } else {
      imm
    }
  }

  #[inline(always)]
  fn padding(&mut self, count: usize) {
    for _ in 0..count {
      self.buffer.char(b' ');
    }
  }

  pub fn as_string(self) -> String {
    String::from_utf8_lossy(&self.finish()).into_owned()
  }

  // High-level formatting methods
  pub fn format_tokens(&mut self, tokens: &TokenBuffer, source: &str) {
    self.token_header();

    let token_count = tokens.kinds.len();

    for i in 0..token_count {
      let token = &tokens.kinds[i];
      let start = tokens.starts[i] as usize;
      let length = tokens.lengths[i] as usize;
      let end = start + length;

      let kind = format!("{token:?}");
      let lexeme = &source[start..end];
      let is_last = i == token_count - 1;

      self.token_row(&kind, lexeme, start as u32, end as u32, is_last);
    }
  }

  pub fn format_tree(&mut self, tree: &Tree, source: &str) {
    self.tree_header(tree.nodes.len());

    // Find the root node - it's the node that isn't a child of any other node
    let mut is_child = vec![false; tree.nodes.len()];

    // Mark all child nodes
    for node in &tree.nodes {
      if node.child_count > 0 {
        let child_start = node.child_start as usize;
        let child_end = child_start + node.child_count as usize;

        for child_idx in child_start..child_end {
          if child_idx < is_child.len() {
            is_child[child_idx] = true;
          }
        }
      }
    }

    // Find all root nodes (nodes that aren't children)
    let mut roots = Vec::new();

    for (i, &is_child_node) in is_child.iter().enumerate() {
      if !is_child_node {
        roots.push(i);
      }
    }

    // Print all root nodes
    let printed = vec![false; tree.nodes.len()];

    for (i, &root_idx) in roots.iter().enumerate() {
      let is_last = i == roots.len() - 1;

      self.print_tree_node(Printee {
        tree,
        node_idx: root_idx,
        source,
        depth: 0,
        is_last,
        parent_continues: Vec::new(),
        printed: printed.to_vec(),
      });
    }
  }

  fn print_tree_node(&mut self, printee: Printee) {
    let node_idx = printee.node_idx;
    let tree = printee.tree;
    let mut printed = printee.printed;

    if node_idx >= tree.nodes.len() || printed[node_idx] {
      return;
    }

    printed[node_idx] = true;

    let node = &tree.nodes[node_idx];
    let parent_continues = printee.parent_continues;

    // Print indentation for parent levels
    for continues in &parent_continues {
      if *continues {
        self.buffer.str("│   ");
      } else {
        self.buffer.str("    ");
      }
    }

    let is_last = printee.is_last;

    // Print branch character for this level
    if is_last {
      self.buffer.str("└── ");
    } else {
      self.buffer.str("├── ");
    }

    // Print node index
    self.buffer.u32(node_idx as u32);
    self.buffer.str(". ");

    // Get token name with value if applicable
    let token_name = if tree.spans.len() > node_idx {
      let span = tree.spans[node_idx];
      let start = span.start as usize;
      let end = start + span.len as usize;

      match node.token {
        Token::Ident | Token::Int | Token::Float | Token::String => {
          let value = &printee.source[start..end];

          format!("{:?}({value})", node.token)
        }
        _ => format!("{:?}", node.token),
      }
    } else {
      format!("{:?}", node.token)
    };

    self.buffer.str(&token_name);
    self.buffer.newline();

    // Print children if any
    if node.child_count > 0 {
      let child_start = node.child_start as usize;
      let child_end = child_start + node.child_count as usize;

      // Build parent continuation state for children
      let mut new_parent_continues = parent_continues.to_vec();
      new_parent_continues.push(!is_last);

      for (i, child_idx) in (child_start..child_end).enumerate() {
        if printed[child_idx] {
          continue;
        }

        let is_last_child = i == node.child_count as usize - 1;

        self.print_tree_node(Printee {
          tree,
          node_idx: child_idx,
          source: printee.source,
          depth: printee.depth + 1,
          is_last: is_last_child,
          parent_continues: new_parent_continues.to_vec(),
          printed: printed.to_vec(),
        });
      }
    }
  }
}
impl Default for PrettyPrinter {
  fn default() -> Self {
    Self::new()
  }
}
