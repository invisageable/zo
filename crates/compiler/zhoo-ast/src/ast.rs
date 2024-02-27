use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Program {
  pub items: Vec<Item>,
}

impl Program {
  #[inline]
  pub fn new() -> Self {
    Self {
      items: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn add_item(&mut self, item: Item) {
    self.items.push(item);
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
  pub kind: ItemKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ItemKind {
  Fun(Fun),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fun {
  pub body: Block,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
  pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stmt {
  pub kind: StmtKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum StmtKind {
  Expr(Expr),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Expr {
  pub kind: ExprKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ExprKind {
  Int(u64),
  Float(f64),
  Ident(String),
}
