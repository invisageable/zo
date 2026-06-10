//! ```sh
//! cargo test -p zo-executor --lib tests::naming
//! ```
//!
//! Naming-convention warnings — one test per declaration site the
//! executor forwards to `zo-checker`.

use super::common::{assert_execution_error, assert_no_errors};

use zo_error::ErrorKind;

#[test]
fn warns_on_camel_case_imu_binding() {
  assert_execution_error(
    r#"fun main() {
  imu myCount := 1;

  showln("{myCount}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_pascal_case_mut_binding() {
  assert_execution_error(
    r#"fun main() {
  mut Total := 0;

  Total += 1;
  showln("{Total}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_lowercase_val_constant() {
  assert_execution_error(
    r#"val max_size: int = 64;

fun main() {
  showln("{max_size}");
}"#,
    ErrorKind::NonScreamingCaseName,
  );
}

#[test]
fn warns_on_pascal_case_function_name() {
  assert_execution_error(
    r#"fun Compute() -> int { 1 }

fun main() {
  showln("{Compute()}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_camel_case_function_argument() {
  assert_execution_error(
    r#"fun double(theValue: int) -> int { theValue * 2 }

fun main() {
  showln("{double(2)}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_snake_case_struct_name() {
  assert_execution_error(
    r#"struct my_point {
  x: int,
}

fun main() {
  imu p := my_point { x = 1 };

  showln("{p.x}");
}"#,
    ErrorKind::NonPascalCaseName,
  );
}

#[test]
fn warns_on_camel_case_struct_field() {
  assert_execution_error(
    r#"struct Point {
  posX: int,
}

fun main() {
  imu p := Point { posX = 1 };

  showln("{p.posX}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_snake_case_enum_name() {
  assert_execution_error(
    r#"enum traffic_light {
  Red,
  Green,
}

fun main() {
  imu light := traffic_light::Red;

  showln("ok");
}"#,
    ErrorKind::NonPascalCaseName,
  );
}

#[test]
fn warns_on_snake_case_type_alias() {
  assert_execution_error(
    r#"type idx = int;

fun main() {
  imu i: idx = 3;

  showln("{i}");
}"#,
    ErrorKind::NonPascalCaseName,
  );
}

#[test]
fn warns_on_snake_case_generic_param() {
  assert_execution_error(
    r#"fun identity<$item>(x: $item) -> $item { x }

fun main() {
  showln("{identity(1)}");
}"#,
    ErrorKind::NonPascalCaseName,
  );
}

#[test]
fn warns_on_pascal_case_abstract_function() {
  assert_execution_error(
    r#"abstract Display {
  fun Show(self) -> str;
}

fun main() {
  showln("ok");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn warns_on_camel_case_tuple_pattern_binding() {
  assert_execution_error(
    r#"fun main() {
  imu (firstItem, b) := (1, 2);

  showln("{firstItem} {b}");
}"#,
    ErrorKind::NonSnakeCaseName,
  );
}

#[test]
fn convention_following_program_is_clean() {
  // No builtin calls — this harness drives the bare executor
  // without the module preload, so `showln` itself would resolve
  // as undefined. Declarations are what this test is about.
  assert_no_errors(
    r#"val MAX_SIZE: int = 64;

type Idx = int;

struct Point {
  x: int,
  pos_y: int,
}

enum TrafficLight {
  Red,
  Green,
}

abstract Display {
  fun render(self) -> str;
}

fun identity<$T>(value: $T) -> $T { value }

fun main() {
  imu point := Point { x = 1, pos_y = 2 };
  imu r0 := identity(3);
  mut total := 0;

  total += point.x + point.pos_y + r0 + MAX_SIZE;
}"#,
  );
}
