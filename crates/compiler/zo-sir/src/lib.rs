mod sir;
pub mod validator;

pub use sir::{
  BinOp, ComputedBinding, Insn, LinkEntry, LinkPath, LinkResolution, LinkSpec,
  ListBinding, ListItemCmd, LoadSource, NurseryKind, Sir, SpawnKind,
  TemplateBindings, UnOp,
};
pub use validator::{ValidationReport, Violation, ViolationKind, validate};
