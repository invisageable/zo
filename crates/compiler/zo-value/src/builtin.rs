use super::value::{Args, Value};

use zo_core::Result;

use smol_str::SmolStr;

pub type BuiltinFn = fn(Args) -> Result<Value>;

#[derive(Clone, Debug)]
pub struct Builtin {
  pub name: SmolStr,
  pub builtin: Value,
}

pub fn print(args: Args) -> Result<Value> {
  for arg in &args.0 {
    println!("{}", arg.value);
  }

  Ok(Value::UNIT)
}
