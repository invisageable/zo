use super::value::{Args, Value, ValueKind};

use zo_interner::interner::{symbol::Symbol, Interner};
use zo_reporter::Result;

use swisskit::span::Span;

use hashbrown::HashMap;
use smol_str::SmolStr;

pub type BuiltinFn = fn(Args) -> Result<Value>;

/// The representation of a builtin function.
#[derive(Clone, Debug)]
pub struct Builtin {
  /// The name of the function.
  pub name: SmolStr,
  /// The builtin function.
  pub builtin: Value,
}

pub fn lookup_builtin(interner: &Interner, sym: Symbol) -> Option<Value> {
  let name = interner.lookup(*sym);

  BUILTINS.get(name).map(|b| b.builtin.clone())
}

lazy_static::lazy_static! {
  pub static ref BUILTINS: HashMap<SmolStr, Builtin> = HashMap::from([
    (SmolStr::new_inline("show"), Builtin {
      name: SmolStr::new_inline("show"),
      builtin: Value::new(ValueKind::Builtin(show), Span::ZERO), // #1.
    }),
    (SmolStr::new_inline("showln"), Builtin {
      name: SmolStr::new_inline("showln"),
      builtin: Value::new(ValueKind::Builtin(showln), Span::ZERO), // #1.
    })
  ]);
}

/// Prints values to the io.
pub fn show(values: Args) -> Result<Value> {
  for value in values.iter() {
    print!("{value}");
  }

  Ok(Value::UNIT)
}

/// Prints values to the io with a new line at the end.
pub fn showln(values: Args) -> Result<Value> {
  for value in values.iter() {
    println!("{value}");
  }

  Ok(Value::UNIT)
}
