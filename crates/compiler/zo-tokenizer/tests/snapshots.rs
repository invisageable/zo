//! ```sh
//! INSTA_UPDATE=1 cargo test -p zo-tokenizer --test snapshots
//! ```

use zo_tokenizer::Tokenizer;

fn assert_yaml_snapshot(name: &str, code: &str) {
  let tokenizer = Tokenizer::new(code);
  let tokenization = tokenizer.tokenize();

  insta::assert_yaml_snapshot!(name, tokenization);
}

#[test]
fn snapshot_realistic_program() {
  assert_yaml_snapshot(
    "realistic_program",
    r#"
    pack main;

    load std::io::(read, readln);
    load math;

    struct Point {
      x: f32,
      y: f32,
    }

    pub fun distance(p1: Point, p2: Point) -> f32 {
      imu dx := p1.x - p2.x;
      imu dy := p1.y - p2.y;

      (dx * dx + dy * dy) as f32
    }
  "#,
  );
}

#[test]
fn snapshot_template() {
  assert_yaml_snapshot(
    "template",
    r#"
    fun main() {
      mut count: int = 0;

      imu counter: </> ::= <>
        <button onclick={fn() => count -= 1}>-</button>
        -- this is a comment.
        {count}
        simple text to see if it work.
        <button onclick={fn() => count += 1}>+</button>
      </>;
      
      #dom counter;
    }
  "#,
  );
}

#[test]
fn snapshot_nested_templates() {
  assert_yaml_snapshot(
    "nested_templates",
    r#"
    fun main() {
      imu view ::= <div class="container">
        {items.map(fn(item) => 
          <li key={item.id}>
            <span>{item.name}</span>
            <button onclick={fn() => delete(item)}>×</button>
          </li>
        )}
      </div>;

      #dom view;
    }
  "#,
  );
}
