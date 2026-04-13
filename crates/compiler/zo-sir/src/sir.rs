use zo_interner::Symbol;
use zo_token::Token;
use zo_ty::Mutability;
use zo_ty::TyId;
use zo_ui_protocol::{Attr, StyleScope, UiCommand};
use zo_value::{FunctionKind, Pubness, ValueId};

/// Reactive bindings carried by `Insn::Template`. Split by
/// target kind so the runtime can dispatch patches without
/// introspecting variant shapes at every step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TemplateBindings {
  /// Text-content bindings: each entry is `(cmd_idx, var)`
  /// where `cmd_idx` points at a `UiCommand::Text(_)` whose
  /// string is regenerated from the state cell for `var` on
  /// each reactive update.
  pub text: Vec<(usize, Symbol)>,
  /// Element-attribute bindings: each entry is `(cmd_idx,
  /// Attr::Dynamic { name, var, initial })` pointing at a
  /// `UiCommand::Element` whose attribute `name` is reactive
  /// on variable `var`. The runtime calls
  /// `UiCommand::set_attr` to apply the patch.
  pub attrs: Vec<(usize, Attr)>,
}

/// Source of a Load instruction — either a function parameter
/// or a local variable on the stack.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadSource {
  /// Function parameter by index (X0-X7 or D0-D7).
  Param(u32),
  /// Local variable by symbol (stack-allocated).
  Local(Symbol),
}

/// Represents a semantic intermediate representation.
#[derive(Debug)]
pub struct Sir {
  /// The linear array of SIR instructions.
  pub instructions: Vec<Insn>,
  /// The next value ID for SSA.
  pub next_value_id: u32,
  /// The next label ID for branch targets.
  pub next_label_id: u32,
}

impl Sir {
  /// Creates a new [`SirBuilder`] instance.
  pub fn new() -> Self {
    Self {
      instructions: Vec::with_capacity(1024),
      next_value_id: 0,
      next_label_id: 0,
    }
  }

  /// Allocates a fresh label ID.
  pub fn next_label(&mut self) -> u32 {
    let id = self.next_label_id;

    self.next_label_id += 1;

    id
  }

  /// Emits an instruction and return its result [`ValueId`].
  ///
  /// Every value-producing instruction has an explicit `dst`
  /// field. Non-value instructions return a sentinel.
  pub fn emit(&mut self, insn: Insn) -> ValueId {
    let value_id = match &insn {
      // All value-producing instructions have explicit dst.
      Insn::ConstInt { dst, .. }
      | Insn::ConstFloat { dst, .. }
      | Insn::ConstBool { dst, .. }
      | Insn::ConstString { dst, .. }
      | Insn::Call { dst, .. }
      | Insn::Load { dst, .. }
      | Insn::BinOp { dst, .. }
      | Insn::UnOp { dst, .. }
      | Insn::ArrayLiteral { dst, .. }
      | Insn::ArrayIndex { dst, .. }
      | Insn::ArrayLen { dst, .. }
      | Insn::ArrayPop { dst, .. }
      | Insn::TupleLiteral { dst, .. }
      | Insn::TupleIndex { dst, .. }
      | Insn::EnumConstruct { dst, .. }
      | Insn::StructConstruct { dst, .. } => *dst,
      // Template uses `id` as its value.
      Insn::Template { id, .. } => *id,
      // Non-value instructions.
      _ => ValueId(u32::MAX),
    };

    self.instructions.push(insn);

    value_id
  }

  /// Offsets all `ValueId`s in instructions by `offset`.
  /// Used when prepending module SIR to avoid ID collisions.
  pub fn offset_value_ids(instructions: &mut [Insn], offset: u32) {
    for insn in instructions.iter_mut() {
      insn.visit_value_ids_mut(&mut |v| v.0 += offset);
    }
  }
}

impl Insn {
  /// Walks every `ValueId` in this instruction, applying `f`.
  /// Used by SIR passes that need to rewrite value IDs
  /// (e.g., module merging, monomorphization).
  pub fn visit_value_ids_mut(&mut self, f: &mut impl FnMut(&mut ValueId)) {
    match self {
      Insn::ConstInt { dst, .. }
      | Insn::ConstFloat { dst, .. }
      | Insn::ConstBool { dst, .. }
      | Insn::ConstString { dst, .. }
      | Insn::Load { dst, .. } => f(dst),
      Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::EnumDef { .. }
      | Insn::StructDef { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. }
      | Insn::FunDef { .. }
      | Insn::ConstDef { .. }
      | Insn::StyleSheet { .. }
      | Insn::Nop => {}
      Insn::VarDef { init, .. } => {
        if let Some(v) = init {
          f(v);
        }
      }
      Insn::Store { value, .. } => f(value),
      Insn::Return { value, .. } => {
        if let Some(v) = value {
          f(v);
        }
      }
      Insn::Call { dst, args, .. } => {
        f(dst);
        args.iter_mut().for_each(&mut *f);
      }
      Insn::BinOp { dst, lhs, rhs, .. } => {
        f(dst);
        f(lhs);
        f(rhs);
      }
      Insn::UnOp { dst, rhs, .. } => {
        f(dst);
        f(rhs);
      }
      Insn::BranchIfNot { cond, .. } => f(cond),
      Insn::Directive { value, .. } => f(value),
      Insn::Template { id, .. } => f(id),
      Insn::ArrayLiteral { dst, elements, .. } => {
        f(dst);
        elements.iter_mut().for_each(&mut *f);
      }
      Insn::ArrayIndex {
        dst, array, index, ..
      } => {
        f(dst);
        f(array);
        f(index);
      }
      Insn::ArrayStore {
        array,
        index,
        value,
        ..
      } => {
        f(array);
        f(index);
        f(value);
      }
      Insn::ArrayLen { dst, array, .. } => {
        f(dst);
        f(array);
      }
      Insn::ArrayPush { array, value, .. } => {
        f(array);
        f(value);
      }
      Insn::ArrayPop { dst, array, .. } => {
        f(dst);
        f(array);
      }
      Insn::TupleLiteral { dst, elements, .. } => {
        f(dst);
        elements.iter_mut().for_each(&mut *f);
      }
      Insn::TupleIndex { dst, tuple, .. } => {
        f(dst);
        f(tuple);
      }
      Insn::EnumConstruct { dst, fields, .. } => {
        f(dst);
        fields.iter_mut().for_each(&mut *f);
      }
      Insn::StructConstruct { dst, fields, .. } => {
        f(dst);
        fields.iter_mut().for_each(&mut *f);
      }
      Insn::FieldStore { base, value, .. } => {
        f(base);
        f(value);
      }
    }
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
  ConstInt {
    dst: ValueId,
    value: u64,
    ty_id: TyId,
  },
  /// Constant float literal.
  ConstFloat {
    dst: ValueId,
    value: f64,
    ty_id: TyId,
  },
  /// Constant boolean value
  ConstBool {
    dst: ValueId,
    value: bool,
    ty_id: TyId,
  },
  /// Constant string value (interned as Symbol).
  ConstString {
    dst: ValueId,
    symbol: Symbol,
    ty_id: TyId,
  },
  /// Variable definition (compile-time binding).
  VarDef {
    name: Symbol,
    ty_id: TyId,
    init: Option<ValueId>,
    mutability: Mutability,
    pubness: Pubness,
  },
  /// Compile-time constant definition: `val X: int = 42;`
  /// No stack slot — value is inlined at every use site.
  ConstDef {
    name: Symbol,
    ty_id: TyId,
    value: ValueId,
    pubness: Pubness,
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
    params: Vec<(Symbol, TyId)>,
    return_ty: TyId,
    body_start: u32,
    kind: FunctionKind,
    pubness: Pubness,
  },
  /// Return from function
  Return {
    value: Option<ValueId>, // None for void returns
    ty_id: TyId,
  },
  /// Function call
  Call {
    dst: ValueId,
    name: Symbol,
    args: Vec<ValueId>,
    ty_id: TyId, // Return type
  },
  /// Load a parameter or local into an SSA value.
  Load {
    dst: ValueId,
    src: LoadSource,
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
  UnOp {
    dst: ValueId,
    op: UnOp,
    rhs: ValueId,
    ty_id: TyId,
  },
  /// Directive execution (e.g., #dom, #run)
  Directive {
    name: Symbol,
    value: ValueId,
    ty_id: TyId,
  },
  /// Module import — resolved at compile time.
  ModuleLoad {
    path: Vec<Symbol>,
    imported_symbols: Vec<Symbol>,
  },
  /// Pack declaration — defines a namespace.
  PackDecl { name: Symbol, pubness: Pubness },
  /// The branch target label.
  Label { id: u32 },
  /// The unconditional jump to a label.
  Jump { target: u32 },
  /// The conditional branch — jump to target if false.
  BranchIfNot { cond: ValueId, target: u32 },
  /// Array literal: [e0, e1, ..., eN].
  ArrayLiteral {
    dst: ValueId,
    elements: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Array index: arr[idx].
  ArrayIndex {
    dst: ValueId,
    array: ValueId,
    index: ValueId,
    ty_id: TyId,
  },
  /// Array length: arr.len.
  ArrayLen {
    dst: ValueId,
    array: ValueId,
    ty_id: TyId,
  },
  /// Array push: arr.push(value). Side effect — mutates len.
  ArrayPush {
    array: ValueId,
    value: ValueId,
    ty_id: TyId,
  },
  /// Array pop: val = arr.pop(). Decrements len, returns last.
  ArrayPop {
    dst: ValueId,
    array: ValueId,
    ty_id: TyId,
  },
  /// Tuple literal: (e0, e1, ..., eN).
  TupleLiteral {
    dst: ValueId,
    elements: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Tuple/struct field read: tup.N (compile-time index).
  TupleIndex {
    dst: ValueId,
    tuple: ValueId,
    index: u32,
    ty_id: TyId,
  },
  /// Struct field write: struct.N = value.
  FieldStore {
    base: ValueId,
    index: u32,
    value: ValueId,
    ty_id: TyId,
  },
  /// Array element write: arr[i] = value.
  ArrayStore {
    array: ValueId,
    index: ValueId,
    value: ValueId,
    ty_id: TyId,
  },
  /// Enum type definition.
  EnumDef {
    name: Symbol,
    ty_id: TyId,
    /// (variant_name, discriminant, field_types).
    variants: Vec<(Symbol, u32, Vec<TyId>)>,
    pubness: Pubness,
  },
  /// Enum variant construction: `Foo::Ok(42)`.
  EnumConstruct {
    dst: ValueId,
    enum_name: Symbol,
    variant: u32,
    fields: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Struct type definition.
  StructDef {
    name: Symbol,
    ty_id: TyId,
    /// (field_name, field_ty, has_default).
    fields: Vec<(Symbol, TyId, bool)>,
    pubness: Pubness,
  },
  /// Struct construction: `Span { lo: 0, hi: 10 }`.
  StructConstruct {
    dst: ValueId,
    struct_name: Symbol,
    fields: Vec<ValueId>,
    ty_id: TyId,
  },
  /// Template literal (fragment or HTML tag)
  Template {
    id: ValueId,
    name: Option<Symbol>,
    ty_id: TyId,
    commands: Vec<UiCommand>,
    /// Reactive bindings. When any bound variable changes,
    /// the listed command(s) must be re-patched with the new
    /// value. Text bindings target `UiCommand::Text(_)`
    /// content; attribute bindings target a named attribute
    /// on a `UiCommand::Element` — the runtime uses
    /// `UiCommand::set_attr` to apply the update.
    bindings: TemplateBindings,
  },
  /// Stylesheet declaration: `$: { ... }` or `pub $: { ... }`.
  StyleSheet {
    css: String,
    scope: StyleScope,
    scope_hash: Option<String>,
  },
  /// Dead instruction — replaces folded operands in-place
  /// so instruction indices stay stable.
  Nop,
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
  /// `++` (string concatenation)
  Concat,
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
      table[Token::PlusPlus as usize] = Some(BinOp::Concat);
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
  // Bitwise not — ``.
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
