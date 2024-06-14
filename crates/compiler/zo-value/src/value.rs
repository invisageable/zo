//! ...

use zo_ast::ast::{Block, Prototype};

use zo_core::interner::symbol::Symbol;

use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Value {
  pub kind: ValueKind,
}

impl Value {
  pub const UNIT: Self = Self {
    kind: ValueKind::Unit,
  };

  #[inline]
  pub fn new(kind: ValueKind) -> Self {
    Self { kind }
  }

  #[inline]
  pub fn int(int: i64) -> Self {
    Self::new(ValueKind::Int(int))
  }

  #[inline]
  pub fn float(float: f64) -> Self {
    Self::new(ValueKind::Float(float))
  }

  #[inline]
  pub fn bool(boolean: bool) -> Self {
    Self::new(ValueKind::Bool(boolean))
  }

  #[inline]
  pub fn char(ch: char) -> Self {
    Self::new(ValueKind::Char(ch))
  }

  #[inline]
  pub fn str(string: SmolStr) -> Self {
    Self::new(ValueKind::Str(string))
  }

  #[inline]
  pub fn fun(prototype: Prototype, block: Block) -> Self {
    Self::new(ValueKind::Fn(prototype, block))
  }

  #[inline]
  pub fn symbolize(&self) -> Symbol {
    self.kind.symbolize()
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    self.kind.as_bool()
  }
}

#[derive(Clone, Debug)]
pub enum ValueKind {
  Unit,
  Int(i64),
  Float(f64),
  Bool(bool),
  Char(char),
  Str(SmolStr),
  Fn(Prototype, Block),
  Return(Box<Value>),
  Builtin(()),
}

impl ValueKind {
  #[inline]
  pub fn symbolize(&self) -> Symbol {
    todo!()
  }

  #[inline]
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Bool(boolean) => *boolean,
      Self::Unit => false,
      _ => false,
    }
  }
}

#[derive(Clone, Debug)]
pub struct Args(pub Vec<Arg>);

impl Args {
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(0usize))
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  #[inline]
  pub fn add_arg(&mut self, arg: Arg) {
    self.0.push(arg)
  }
}

impl std::ops::Deref for Args {
  type Target = Vec<Arg>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Clone, Debug)]
pub struct Arg {
  pub value: Value,
  // pub span: Span,
}
