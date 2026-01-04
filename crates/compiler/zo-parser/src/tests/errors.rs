use crate::tests::common::assert_error;

use zo_error::ErrorKind;

#[test]
fn test_mismatched_tags() {
  assert_error(
    r#"
      fun main() {
        imu component ::= <div>Content</span>;
      }
    "#,
    ErrorKind::MismatchedTags,
  );
}

// #[test]
// fn test_unclosed_element() {
//   assert_error(
//     r#"
//       fun main() {
//         imu component ::= <div>Content;
//       }
//     "#,
//     ErrorKind::UnclosedElement,
//   );
// }

// #[test]
// fn test_unclosed_fragment() {
//   assert_error(
//     r#"
//       fun main() {
//         imu component ::= <>Content;
//       }
//     "#,
//     ErrorKind::UnclosedFragment,
//   );
// }

// #[test]
// fn test_invalid_attribute_value() {
//   assert_error(
//     r#"
//       fun main() {
//         imu component ::= <div class= />;
//       }
//     "#,
//     ErrorKind::InvalidAttributeValue,
//   );
// }

// #[test]
// fn test_assignment_as_expr() {
//   assert_nodes_stream(
//     r#"
//       fun main() {
//         mut x: int = 0;
//         imu result := (x = 42);
//       }
//     "#,
//     &[],
//   );
// }

// #[test]
// fn test_compound_assignment_as_expr() {
//   assert_nodes_stream(
//     r#"
//       fun main() {
//         mut count: int = 0;
//         imu increment := count += 1;
//       }
//     "#,
//     &[],
//   );
// }

// #[test]
// fn test_compound_assignment_in_closure() {
//   assert_nodes_stream(
//     r#"
//       fun main() {
//         mut count: int = 0;
//         imu incrementer := fn() -> count += 1;
//       }
//     "#,
//     &[],
//   );
// }
