//! ...

use cranelift_module::FuncId;

#[derive(Debug)]
pub(crate) struct Function {
  pub defined: bool,
  pub id: FuncId,
  pub param_count: usize,
}
