use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Program {
  pub items: Vec<Item>,
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
