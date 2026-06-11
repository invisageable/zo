use crate::tests::common::{assert_no_errors, assert_sir_structure};

use zo_sir::Insn;
use zo_ui_protocol::{StyleScope, UiCommand};

// === STYLE BLOCK COMPILATION ===

#[test]
fn test_style_scoped_emits_stylesheet_in_template() {
  assert_sir_structure(
    r#"
      $: {
        p {
          color: cyan;
          fw: 800;
        }
      }

      fun main() {
        imu view: </> ::= <p>styled text</p>;
        #render view;
      }
    "#,
    |sir| {
      // Find the Template instruction.
      let template = sir.iter().find_map(|i| match i {
        Insn::Template { commands, .. } => Some(commands),
        _ => None,
      });

      let commands = template.expect("should emit Template SIR");

      // First command should be the StyleSheet.
      let style_cmd = commands
        .iter()
        .find(|c| matches!(c, UiCommand::StyleSheet { .. }));

      let style_cmd = style_cmd.expect("template should contain StyleSheet");

      match style_cmd {
        UiCommand::StyleSheet {
          css,
          scope,
          scope_hash,
        } => {
          assert_eq!(*scope, StyleScope::Scoped);
          assert!(
            css.contains("color: cyan;"),
            "CSS should contain 'color: cyan;', got: {css}"
          );
          assert!(
            css.contains("font-weight: 800;"),
            "fw should resolve to font-weight, got: {css}"
          );
          // Scoped: selector should have a hash suffix.
          assert!(
            css.contains("._zo_"),
            "scoped selector should have hash, got: {css}"
          );
          // scope_hash should be present for scoped.
          assert!(
            scope_hash.is_some(),
            "scoped stylesheet should have scope_hash"
          );
          let hash = scope_hash.as_ref().unwrap();
          assert!(
            hash.starts_with("_zo_"),
            "scope_hash should start with _zo_, got: {hash}"
          );
        }
        _ => unreachable!(),
      }
    },
  );
}

#[test]
fn test_style_global_no_scope_hash() {
  assert_sir_structure(
    r#"
      pub $: {
        html body {
          w: 100%;
          h: 100%;
        }
      }

      fun main() {
        imu view: </> ::= <p>full width</p>;
        #render view;
      }
    "#,
    |sir| {
      let template = sir.iter().find_map(|i| match i {
        Insn::Template { commands, .. } => Some(commands),
        _ => None,
      });

      let commands = template.expect("should emit Template SIR");

      let style_cmd = commands
        .iter()
        .find(|c| matches!(c, UiCommand::StyleSheet { .. }))
        .expect("template should contain StyleSheet");

      match style_cmd {
        UiCommand::StyleSheet { css, scope, .. } => {
          assert_eq!(*scope, StyleScope::Global);
          assert!(
            css.contains("width: 100%;"),
            "w should resolve to width, got: {css}"
          );
          assert!(
            css.contains("height: 100%;"),
            "h should resolve to height, got: {css}"
          );
          // Global: NO scope hash on selectors.
          assert!(
            !css.contains("._zo_"),
            "global selector should NOT have hash, got: {css}"
          );
          assert!(
            css.contains("html body"),
            "compound selector preserved, got: {css}"
          );
        }
        _ => unreachable!(),
      }
    },
  );
}

#[test]
fn test_style_shorthand_resolution() {
  assert_sir_structure(
    r#"
      $: {
        .card {
          p: 16px;
          m: 8px;
          br: 4px;
          bg: #f0f0f0;
        }
      }

      fun main() {
        imu view: </> ::= <div>content</div>;
        #render view;
      }
    "#,
    |sir| {
      let template = sir.iter().find_map(|i| match i {
        Insn::Template { commands, .. } => Some(commands),
        _ => None,
      });

      let commands = template.expect("should emit Template SIR");

      let style_cmd = commands
        .iter()
        .find(|c| matches!(c, UiCommand::StyleSheet { .. }))
        .expect("template should contain StyleSheet");

      match style_cmd {
        UiCommand::StyleSheet { css, .. } => {
          assert!(css.contains("padding: 16px;"), "p -> padding, got: {css}");
          assert!(css.contains("margin: 8px;"), "m -> margin, got: {css}");
          assert!(
            css.contains("border-radius: 4px;"),
            "br -> border-radius, got: {css}"
          );
          assert!(
            css.contains("background: #f0f0f0;"),
            "bg -> background, got: {css}"
          );
        }
        _ => unreachable!(),
      }
    },
  );
}

#[test]
fn test_style_multiple_rules() {
  assert_sir_structure(
    r#"
      $: {
        .title {
          color: cyan;
          ta: center;
        }

        .subtitle {
          color: gray;
          fs: 14px;
        }
      }

      fun main() {
        imu view: </> ::= <p>hello</p>;
        #render view;
      }
    "#,
    |sir| {
      let template = sir.iter().find_map(|i| match i {
        Insn::Template { commands, .. } => Some(commands),
        _ => None,
      });

      let commands = template.expect("should emit Template SIR");

      let style_cmd = commands
        .iter()
        .find(|c| matches!(c, UiCommand::StyleSheet { .. }))
        .expect("template should contain StyleSheet");

      match style_cmd {
        UiCommand::StyleSheet { css, .. } => {
          assert!(
            css.contains("text-align: center;"),
            "ta -> text-align, got: {css}"
          );
          assert!(
            css.contains("font-size: 14px;"),
            "fs -> font-size, got: {css}"
          );
          assert!(css.contains(".title"), "first selector present, got: {css}");
          assert!(
            css.contains(".subtitle"),
            "second selector present, got: {css}"
          );
        }
        _ => unreachable!(),
      }
    },
  );
}

#[test]
fn test_style_block_no_errors() {
  assert_no_errors(
    r#"
      $: {
        p { color: cyan; fw: 800; }
      }

      fun main() {
        imu view: </> ::= <p>hello</p>;
        #render view;
      }
    "#,
  );
}

#[test]
fn test_style_grouped_selector() {
  assert_sir_structure(
    r#"
      pub $: {
        html, body {
          m: 0;
          p: 0;
        }
      }

      fun main() {
        imu view: </> ::= <p>hello</p>;
        #render view;
      }
    "#,
    |sir| {
      let template = sir.iter().find_map(|i| match i {
        Insn::Template { commands, .. } => Some(commands),
        _ => None,
      });

      let commands = template.expect("should emit Template SIR");

      let style_cmd = commands
        .iter()
        .find(|c| matches!(c, UiCommand::StyleSheet { .. }))
        .expect("template should contain StyleSheet");

      match style_cmd {
        UiCommand::StyleSheet { css, .. } => {
          assert!(
            css.contains("html, body"),
            "grouped selector preserved, got: {css}"
          );
          assert!(css.contains("margin: 0;"), "m -> margin, got: {css}");
          assert!(css.contains("padding: 0;"), "p -> padding, got: {css}");
        }
        _ => unreachable!(),
      }
    },
  );
}

#[test]
fn pseudo_class_selector_survives_with_hash_before_it() {
  // `.btn:hover` must reach the stylesheet with the scope hash
  // inserted BEFORE the pseudo suffix (`.btn._zo_x:hover`) — hash
  // after the pseudo would still match in a browser, but the
  // native key/state split reads `base:pseudo` left to right.
  assert_sir_structure(
    r#"
      $: {
        .btn {
          bg: blue;
        }
        .btn:hover {
          bg: red;
        }
        .btn:active {
          bg: green;
        }
      }

      fun main() {
        imu view: </> ::= <button class="btn">go</button>;
        #render view;
      }
    "#,
    |sir| {
      let css = sir
        .iter()
        .find_map(|i| match i {
          Insn::StyleSheet { css, .. } => Some(css.as_str()),
          _ => None,
        })
        .expect("style block should emit Insn::StyleSheet");

      let hover_at = css.find(":hover").expect("hover rule lost");
      let hash_at = css.find("._zo_").expect("scope hash missing");

      assert!(
        hash_at < hover_at,
        "hash must precede the pseudo suffix: {css}"
      );
      assert!(css.contains(":active"), "active rule lost: {css}");

      // The compiled form is parseable by the native rule model and
      // keys by state.
      let parsed = zo_ui_protocol::style::css::parse(css);
      let hover = zo_ui_protocol::style::css::author_state_patch(
        &parsed.rules,
        ".btn",
        zo_ui_protocol::style::css::Interaction::Hover,
      );

      assert!(hover.is_some(), "native model lost the hover rule: {css}");
    },
  );
}
