//! ```sh
//! INSTA_UPDATE=1 cargo test -p zo-parser --test snapshots
//! ``

use zo_parser::Parser;
use zo_tokenizer::Tokenizer;

pub fn assert_yaml_snapshot(name: &str, code: &str) {
  let tokenizer = Tokenizer::new(code);
  let tokenization = tokenizer.tokenize();

  let parser = Parser::new(&tokenization, code);
  let parsing = parser.parse();

  insta::assert_yaml_snapshot!(name, parsing);
}

#[test]
fn snapshot_simple_function() {
  assert_yaml_snapshot(
    "simple_function",
    r#"
      fun add(a: int, b: int) -> int {
        return a + b;
      }

      fun main() {
        imu adding: int = add(40, 2);

        showln(adding);
      }
    "#,
  );
}

#[test]
fn snapshot_template_with_interpolation() {
  assert_yaml_snapshot(
    "template_with_interpolation",
    r#"
      fun main() {
        imu component ::= <div class={if enabled {"on"} else {"off"}} />;

        #dom component;
      }
    "#,
  );
}

#[test]
fn test_fragment_with_multiple_elements() {
  assert_yaml_snapshot(
    "fragment_with_multiple_elements",
    r#"
      fun main() {
        imu component ::= <>
          <header>Header</header>
          <main>Content</main>
          <footer>Footer</footer>
        </>;

        #dom component;
      }
    "#,
  );
}

#[test]
fn test_mixed_code() {
  assert_yaml_snapshot(
    "mixed_code",
    r#"
      fun main() {
        imu name: str = "johndoe";

        imu component ::= <div>
          Welcome {name}!
          <br />
          How are you today?
        </div>;

        #dom component;
      }
    "#,
  );
}
