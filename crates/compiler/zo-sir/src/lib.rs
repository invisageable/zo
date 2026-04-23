mod sir;
pub mod validator;

pub use sir::{
  BinOp, Insn, LoadSource, Sir, SpawnKind, TemplateBindings, UnOp,
};
pub use validator::{ValidationReport, Violation, ViolationKind, validate};
