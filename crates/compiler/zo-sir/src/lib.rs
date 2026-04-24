mod sir;
pub mod validator;

pub use sir::{
  BinOp, Insn, LoadSource, NurseryKind, Sir, SpawnKind, TemplateBindings, UnOp,
};
pub use validator::{ValidationReport, Violation, ViolationKind, validate};
