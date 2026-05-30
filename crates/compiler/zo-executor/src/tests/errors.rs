use crate::tests::common::{
  execution_diagnostics, execution_errors, span_text,
};

use zo_error::ErrorKind;
use zo_reporter::Detail;
use zo_span::Span;

/// A call with the wrong arity records the callee, the
/// expected/given counts, and the rendered signature.
#[test]
fn test_arg_count_mismatch_carries_signature() {
  let source = "fun add(a: int, b: int) -> int {\n  return a + b;\n}\nfun main() {\n  imu x: int = add(1);\n}";

  let (_errors, details) = execution_diagnostics(source);

  let (callee, expected, given, signature) = details
    .iter()
    .find_map(|(e, d)| match d {
      Detail::ArgCount {
        callee,
        expected,
        given,
        signature,
      } if e.kind() == ErrorKind::ArgumentCountMismatch => {
        Some((callee.clone(), *expected, *given, signature.clone()))
      }
      _ => None,
    })
    .expect("expected an ArgCount detail");

  assert_eq!(&*callee, "add");
  assert_eq!(expected, 2);
  assert_eq!(given, 1);
  assert_eq!(&*signature, "add(a: int, b: int) -> int");
}

/// An argument whose type doesn't match the parameter records
/// the callee, the found/expected types, and the signature.
#[test]
fn test_arg_type_mismatch_carries_signature() {
  let source = "fun greet(name: str) -> str {\n  return name;\n}\nfun main() {\n  imu x: str = greet(42);\n}";

  let (_errors, details) = execution_diagnostics(source);

  let (callee, found, expected, signature) = details
    .iter()
    .find_map(|(e, d)| match d {
      Detail::ArgType {
        callee,
        found,
        expected,
        signature,
      } if e.kind() == ErrorKind::TypeMismatch => Some((
        callee.clone(),
        found.clone(),
        expected.clone(),
        signature.clone(),
      )),
      _ => None,
    })
    .expect("expected an ArgType detail");

  assert_eq!(&*callee, "greet");
  assert_eq!(&*found, "int");
  assert_eq!(&*expected, "str");
  assert_eq!(&*signature, "greet(name: str) -> str");
}

/// An undefined variable that's a near-typo of an in-scope
/// binding suggests the closest name — `cont` → `count`.
#[test]
fn test_undefined_variable_suggests_closest_name() {
  let source = "fun main() {\n  imu count: int = 0;\n  showln(cont);\n}";

  let (_errors, details) = execution_diagnostics(source);

  let suggestion = details
    .iter()
    .find_map(|(e, d)| match d {
      Detail::Suggestion(name) if e.kind() == ErrorKind::UndefinedVariable => {
        Some(name.clone())
      }
      _ => None,
    })
    .expect("expected a suggestion for the undefined variable");

  assert_eq!(&*suggestion, "count");
}

/// A binop mismatch records the conflicting type names —
/// `bool` (primary, the `true`) and `int` (secondary, the
/// `1`) — so the diagnostic can name them.
#[test]
fn test_binop_mismatch_names_types() {
  let source = "fun main() {\n  imu x := 1 + true;\n}";

  let (_errors, details) = execution_diagnostics(source);

  let names = details
    .iter()
    .find_map(|(e, d)| match d {
      Detail::Types(names) if e.kind() == ErrorKind::TypeMismatch => {
        Some(names)
      }
      _ => None,
    })
    .expect("expected a TypeMismatch carrying type names");

  assert_eq!(&*names.primary, "bool");
  assert_eq!(&*names.secondary, "int");
}

/// A type mismatch in an arithmetic binop highlights the two
/// operand values — `1` and `true` in `1 + true` — not the
/// `+` operator.
#[test]
fn test_binop_mismatch_highlights_operands() {
  let source = "fun main() {\n  imu x := 1 + true;\n}";

  let mismatch = execution_errors(source)
    .into_iter()
    .find(|e| e.kind() == ErrorKind::TypeMismatch)
    .expect("expected a TypeMismatch between the operands");

  assert_ne!(mismatch.span(), Span::ZERO);
  assert_eq!(span_text(source, mismatch.span()), "true");

  let secondary = mismatch.secondary_span().expect("expected a secondary");

  assert_eq!(span_text(source, secondary), "1");
}

/// A type mismatch in a logical binop highlights both
/// operands — `true` and `"false"` in `true || "false"`.
#[test]
fn test_logical_binop_mismatch_highlights_operands() {
  let source = "fun main() {\n  imu x := true || \"false\";\n}";

  let mismatch = execution_errors(source)
    .into_iter()
    .find(|e| e.kind() == ErrorKind::TypeMismatch)
    .expect("expected a TypeMismatch between the operands");

  assert_ne!(mismatch.span(), Span::ZERO);
  assert_eq!(span_text(source, mismatch.span()), "\"false\"");

  let secondary = mismatch.secondary_span().expect("expected a secondary");

  assert_eq!(span_text(source, secondary), "true");
}

/// Concatenating a non-`str` with a `str` highlights the
/// offending value (`42`) as primary and the other (`"hi"`)
/// as secondary — not the `++` operator.
#[test]
fn test_concat_mismatch_highlights_operands() {
  let source = "fun main() {\n  imu s: str = 42 ++ \"hi\";\n}";

  let mismatch = execution_errors(source)
    .into_iter()
    .find(|e| e.kind() == ErrorKind::TypeMismatch)
    .expect("expected a TypeMismatch in the concatenation");

  assert_ne!(mismatch.span(), Span::ZERO);
  assert_eq!(span_text(source, mismatch.span()), "42");

  let secondary = mismatch.secondary_span().expect("expected a secondary");

  assert_eq!(span_text(source, secondary), "\"hi\"");
}

/// A function whose body value contradicts its declared
/// return type points at the returned value, not the `fun`
/// keyword.
#[test]
fn test_return_type_mismatch_highlights_value() {
  let source = "fun main() -> str {\n  \"DONE\"\n}";

  let mismatch = execution_errors(source)
    .into_iter()
    .find(|e| e.kind() == ErrorKind::TypeMismatch)
    .expect("expected a TypeMismatch for the return value");

  assert_ne!(mismatch.span(), Span::ZERO);
  assert_eq!(span_text(source, mismatch.span()), "\"DONE\"");
}
