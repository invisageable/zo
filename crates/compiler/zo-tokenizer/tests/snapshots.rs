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

// ── HTML comment stripping ──────────────────────────────────

use zo_token::Token;

fn count_tokens(code: &str) -> std::collections::HashMap<Token, usize> {
  let tokenizer = Tokenizer::new(code);
  let tokenization = tokenizer.tokenize();
  let mut counts: std::collections::HashMap<Token, usize> =
    std::collections::HashMap::new();

  for kind in tokenization.tokens.kinds.iter() {
    *counts.entry(*kind).or_insert(0) += 1;
  }

  counts
}

fn template_text_values(code: &str) -> Vec<String> {
  let tokenizer = Tokenizer::new(code);
  let tokenization = tokenizer.tokenize();
  let mut out = Vec::new();

  for (i, kind) in tokenization.tokens.kinds.iter().enumerate() {
    if *kind == Token::TemplateText {
      let lit_idx = tokenization.tokens.literal_indices[i] as usize;
      let sym = tokenization.literals.identifiers[lit_idx];
      out.push(tokenization.interner.get(sym).to_string());
    }
  }

  out
}

#[test]
fn html_comment_stripped_between_elements() {
  // Single-line HTML comment between two <p> tags should
  // produce zero TemplateText tokens for the comment body
  // and zero LAngle-hanging state.
  let code = r#"
    fun main() {
      imu view ::= <>
        <p>before</p>
        <!-- this is a comment -->
        <p>after</p>
      </>;
    }
  "#;

  let texts = template_text_values(code);
  let joined = texts.join("");

  assert!(
    !joined.contains("this is a comment"),
    "comment body should be stripped, got texts: {:?}",
    texts
  );
  assert!(
    joined.contains("before"),
    "text before comment should survive, got: {:?}",
    texts
  );
  assert!(
    joined.contains("after"),
    "text after comment should survive, got: {:?}",
    texts
  );
}

#[test]
fn html_comment_multiline_stripped() {
  // Multi-line comments must consume across newlines and
  // still return to normal template text afterwards.
  let code = r#"
    fun main() {
      imu view ::= <>
        <!--
          this spans
          multiple lines
        -->
        visible
      </>;
    }
  "#;

  let texts = template_text_values(code);
  let joined = texts.join("");

  assert!(!joined.contains("multiple lines"));
  assert!(!joined.contains("this spans"));
  assert!(joined.contains("visible"));
}

#[test]
fn html_comment_does_not_break_adjacent_elements() {
  // A comment directly adjacent to an element shouldn't
  // leave stray tokens that confuse the parser.
  let code = r#"
    fun main() {
      imu view ::= <><!-- note --><p>hi</p></>;
    }
  "#;

  let counts = count_tokens(code);

  // Expect exactly one pair of LAngle (<p>) + one Slash2 (</)
  // + one TemplateFragmentStart (<>) + one TemplateFragmentEnd
  // (</>). The comment contributes zero tokens.
  assert_eq!(
    counts
      .get(&Token::TemplateFragmentStart)
      .copied()
      .unwrap_or(0),
    1,
    "exactly one <> start"
  );
  assert_eq!(
    counts
      .get(&Token::TemplateFragmentEnd)
      .copied()
      .unwrap_or(0),
    1,
    "exactly one </> end"
  );
}

#[test]
fn unterminated_html_comment_does_not_panic() {
  // A comment with no closing `-->` consumes to EOF but
  // must not crash the tokenizer. The template body is
  // effectively swallowed, which is fine for a malformed
  // input.
  let code = r#"
    fun main() {
      imu view ::= <>
        <!-- unterminated comment
      </>;
    }
  "#;

  let _ = Tokenizer::new(code).tokenize();
}

// ── Context-sensitive comment forms (svelte-style) ─────────

#[test]
fn zo_line_comment_inside_tag_attribute_list() {
  // `--` line comments work between attributes (tag markup
  // context). The comment body does not leak into any
  // TemplateText token.
  let code = r#"
    fun main() {
      imu view ::= <img
        src="a.png"
        -- this is a line comment
        width="128"
      />;
    }
  "#;

  let texts = template_text_values(code);
  let joined = texts.join("");

  assert!(
    !joined.contains("this is a line comment"),
    "line comment should be stripped, got: {:?}",
    texts
  );
}

#[test]
fn zo_block_comment_inside_tag_attribute_list() {
  // `-* ... *-` block comments work between attributes —
  // including across line boundaries.
  let code = r#"
    fun main() {
      imu view ::= <img
        src="a.png"
        -*
          multi-line
          block comment
        *-
        width="128"
      />;
    }
  "#;

  let texts = template_text_values(code);
  let joined = texts.join("");

  assert!(!joined.contains("multi-line"));
  assert!(!joined.contains("block comment"));
}

#[test]
fn zo_line_comment_inside_brace_expression() {
  // `--` line comments work inside `{...}` interpolation
  // expressions. The comment body does not leak into the
  // template text.
  let code = r#"
    fun main() {
      mut count: int = 0;

      imu view ::= <>
        {-- this is a comment inside braces
          count}
      </>;
    }
  "#;

  let texts = template_text_values(code);
  let joined = texts.join("");

  assert!(!joined.contains("this is a comment inside braces"));
}
