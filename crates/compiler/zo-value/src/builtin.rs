use super::value::Value;

use zo_reporter::Result;

use smol_str::SmolStr;

pub type BuiltinFn = fn(Vec<Value>) -> Result<Value>;

/// The representation of a builtin function.
#[derive(Clone, Debug)]
pub struct Builtin {
  /// The name of the function.
  pub name: SmolStr,
  /// The builtin function.
  pub builtin: Value,
}

/// Prints values to the io.
pub fn print(values: Vec<Value>) -> Result<Value> {
  for value in &values {
    println!("{value}");
  }

  Ok(Value::UNIT)
}
