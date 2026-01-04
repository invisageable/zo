use zo_interner::Symbol;
use zo_token::Token;
use zo_ty::TyId;
use zo_ui_protocol::UiCommand;
use zo_value::{Mutability, ValueId};

/// Represents a semantic intermediate representation.
#[derive(Debug)]
pub struct Sir {
  /// The linear array of SIR instructions.
  pub instructions: Vec<Insn>,
  /// The next value ID for SSA.
  pub next_value_id: u32,
}
impl Sir {
  /// Creates a new [`SirBuilder`] instance.
  pub fn new() -> Self {
    Self {
      instructions: Vec::with_capacity(1024),
      next_value_id: 0,
    }
  }

  /// Emits an instruction and return its result [`ValueId`].
  pub fn emit(&mut self, insn: Insn) -> ValueId {
    // For instructions with explicit destinations, return that dst
    let value_id = match &insn {
      Insn::Load { dst, .. } => *dst,
      Insn::BinOp { dst, .. } => *dst,
      // Instructions that don't produce values return a dummy ValueId
      Insn::FunDef { .. }
      | Insn::Return { .. }
      | Insn::VarDef { .. }
      | Insn::Store { .. } => {
        ValueId(u32::MAX) // Sentinel value for non-value-producing instructions
      }
      // Constants and other value-producing instructions
      _ => {
        let id = ValueId(self.next_value_id);

        self.next_value_id += 1;

        id
      }
    };

    self.instructions.push(insn);

    value_id
  }
}
impl Default for Sir {
  fn default() -> Self {
    Self::new()
  }
}

/// SIR Instructions - minimal set for current executor
#[derive(Clone, Debug, PartialEq)]
pub enum Insn {
  /// Constant integer literal.
  ConstInt { value: u64, ty_id: TyId },
  /// Constant float literal.
  ConstFloat { value: f64, ty_id: TyId },
  /// Constant boolean value
  ConstBool { value: bool, ty_id: TyId },
  /// Constant string value (interned as Symbol).
  ConstString { symbol: Symbol, ty_id: TyId },
  /// Variable definition (compile-time binding).
  VarDef {
    name: Symbol,
    ty_id: TyId,
    init: Option<ValueId>,
    mutability: Mutability,
  },
  /// Store to variable/memory
  Store {
    name: Symbol,   // Variable to store to
    value: ValueId, // Value to store
    ty_id: TyId,    // Type of value
  },
  /// Function definition
  FunDef {
    name: Symbol,
    params: Vec<(Symbol, TyId)>, // Parameter names and types
    return_ty: TyId,
    body_start: u32, // Index where function body starts in instruction stream
  },
  /// Return from function
  Return {
    value: Option<ValueId>, // None for void returns
    ty_id: TyId,
  },
  /// Function call
  Call {
    name: Symbol,
    args: Vec<ValueId>,
    ty_id: TyId, // Return type
  },
  /// Load a parameter or local into an SSA value
  Load {
    dst: ValueId, // Destination SSA value
    src: u32,     // Parameter index or local ID
    ty_id: TyId,
  },
  /// Binary operation
  BinOp {
    dst: ValueId, // Destination SSA value
    op: BinOp,
    lhs: ValueId,
    rhs: ValueId,
    ty_id: TyId,
  },
  /// Unary operation
  UnOp { op: UnOp, rhs: ValueId, ty_id: TyId },
  /// Directive execution (e.g., #dom, #run)
  Directive {
    name: Symbol,
    value: ValueId,
    ty_id: TyId,
  },
  /// Template literal (fragment or HTML tag)
  Template {
    id: ValueId,
    name: Option<Symbol>, // None for fragments (<>), Some for tags (<h1>)
    ty_id: TyId,
    commands: Vec<UiCommand>, // UI commands for this template
  },
}

/// Represents binary operators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
  /// `+`
  Add,
  /// `-`
  Sub,
  /// `*`
  Mul,
  /// `/`
  Div,
  /// `%`
  Rem,
  /// `==`
  Eq,
  /// `!=`
  Neq,
  /// `<`
  Lt,
  /// `<=`
  Lte,
  /// `>`
  Gt,
  /// `>=`
  Gte,
  /// `&&`
  And,
  /// `||`
  Or,
  /// `&`
  BitAnd,
  /// `|`
  BitOr,
  /// `^`
  BitXor,
  /// `<<`
  Shl,
  /// `>>`
  Shr,
}
impl BinOp {
  /// Gets the related [`BinOp`] from a [`Token`].
  pub const fn from(&self, kind: Token) -> Option<BinOp> {
    const BINOPS: [Option<BinOp>; 256] = {
      let mut table = [None; 256];
      table[Token::Plus as usize] = Some(BinOp::Add);
      table[Token::Minus as usize] = Some(BinOp::Sub);
      table[Token::Star as usize] = Some(BinOp::Mul);
      table[Token::Slash as usize] = Some(BinOp::Div);
      table[Token::Percent as usize] = Some(BinOp::Rem);
      table[Token::Eq as usize] = Some(BinOp::Eq);
      table[Token::BangEq as usize] = Some(BinOp::Neq);
      table[Token::Lt as usize] = Some(BinOp::Lt);
      table[Token::LtEq as usize] = Some(BinOp::Lte);
      table[Token::Gt as usize] = Some(BinOp::Gt);
      table[Token::GtEq as usize] = Some(BinOp::Gte);
      table[Token::Amp as usize] = Some(BinOp::And);
      table[Token::PipePipe as usize] = Some(BinOp::Or);
      table[Token::PipePipe as usize] = Some(BinOp::BitAnd);
      table[Token::PipePipe as usize] = Some(BinOp::BitOr);
      table[Token::PipePipe as usize] = Some(BinOp::BitXor);
      table[Token::PipePipe as usize] = Some(BinOp::Shl);
      table[Token::PipePipe as usize] = Some(BinOp::Shr);
      table
    };

    BINOPS[kind as usize]
  }
}

/// Represents unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
  // Arithmetic negation — `-x`.
  Neg,
  // Logical not — `!x`.
  Not,
  // Reference — `&x`.
  Ref,
  // Deref — `*x`.
  Deref,
  // Bitwize not — ``.
  BitNot,
}
impl UnOp {
  /// Gets the related [`UnOp`] from a [`Token`].
  pub const fn from(&self, kind: Token) -> Option<UnOp> {
    const UNOPS: [Option<UnOp>; 256] = {
      let mut table = [None; 256];
      table[Token::Bang as usize] = Some(UnOp::Not);
      table[Token::Minus as usize] = Some(UnOp::Neg);
      table[Token::Amp as usize] = Some(UnOp::Ref);
      table[Token::Star as usize] = Some(UnOp::Deref);
      table[Token::Star as usize] = Some(UnOp::BitNot);
      table
    };

    UNOPS[kind as usize]
  }
}
