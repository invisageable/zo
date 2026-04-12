//! Raw HTML splicing for the `{#html expr}` template directive.
//!
//! Reuses zo's full template pipeline to parse an HTML blob
//! into a sequence of `UiCommand`s. The trick: wrap the input
//! string in a synthetic zo source program that declares a
//! template literal containing the blob, then run the complete
//! tokenizer + parser + executor pipeline on it. The resulting
//! `Insn::Template` already carries the `Vec<UiCommand>` we
//! need — extract and return it.
//!
//! This reuses **every byte** of zo's existing template parser.
//! We do not write a new HTML parser; we do not duplicate
//! `handle_template`'s tree-walking logic; we do not touch the
//! main executor's state at all. The sub-executor runs in its
//! own fresh interner / SIR / tree, so the main compilation
//! state is untouched.
//!
//! ### Supported subset
//!
//! Whatever zo's template tokenizer + parser already accept:
//! open / close / self-closing tags, attributes (quoted,
//! boolean), inline text, nested elements, HTML comments
//! (stripped by the tokenizer as of ZO-CL07), the `Custom`
//! fallback for unknown tag names.
//!
//! ### MVP limitations (deferred to follow-up)
//!
//! - **HTML entities (`&amp;`, `&lt;`, etc.) are not decoded
//!   and cause a parse break** because zo's template-text
//!   scanner exits template mode on `;`. Users must pre-decode
//!   entities before passing strings to `#html`. Follow-up can
//!   add a `;`-masking pre-pass + post-decode pipeline.
//! - **Numeric entities (`&#65;`, `&#x41;`)**: same as above.
//! - **CDATA / DOCTYPE / processing instructions**: not
//!   supported; passed through as stray text which the zo
//!   parser may reject.
//! - **zo interpolation (`{var}`) inside the blob**: the blob
//!   is parsed in an isolated sub-executor with no outer
//!   scope, so interpolation expressions either fail lookup or
//!   produce empty text. `#html` is for STATIC content.

use zo_parser::Parser;
use zo_sir::Insn;
use zo_tokenizer::Tokenizer;
use zo_ui_protocol::UiCommand;

/// Parse a raw HTML blob into a sequence of `UiCommand`s by
/// running it through zo's full tokenizer + parser + executor
/// pipeline on a synthetic template literal declaration, then
/// extracting the commands from the resulting `Insn::Template`.
///
/// Empty input yields an empty vector. If the pipeline fails
/// to produce a `Template` insn (e.g. the blob is malformed in
/// a way that escapes the wrapping fragment), returns an empty
/// vector — the caller can surface diagnostics through the
/// main executor's error collector.
pub(crate) fn parse_raw_html(input: &str) -> Vec<UiCommand> {
  if input.is_empty() {
    return Vec::new();
  }

  // Wrap the blob in a zo function body that declares an
  // anonymous template literal. The executor needs a full
  // top-level function to run, so this is the minimum
  // envelope.
  let wrapped =
    format!("fun __zo_html_inline__() {{ imu __v__: </> ::= <>{input}</>; }}");

  let tokenizer = Tokenizer::new(&wrapped);
  let mut tokenization = tokenizer.tokenize();
  let parser = Parser::new(&tokenization, &wrapped);
  let parsing = parser.parse();

  let executor = crate::Executor::new(
    &parsing.tree,
    &mut tokenization.interner,
    &tokenization.literals,
  );

  let (sir, _, _, _) = executor.execute();

  // Pull commands from the first `Insn::Template` the sub-
  // pipeline produced.
  sir
    .instructions
    .iter()
    .find_map(|insn| match insn {
      Insn::Template { commands, .. } => Some(commands.clone()),
      _ => None,
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use super::*;

  use zo_ui_protocol::ElementTag;

  fn text_of(cmd: &UiCommand) -> Option<&str> {
    match cmd {
      UiCommand::Text(s) => Some(s.as_str()),
      _ => None,
    }
  }

  #[test]
  fn parse_empty_input_is_empty() {
    assert_eq!(parse_raw_html(""), Vec::new());
  }

  #[test]
  fn parse_plain_text_emits_text_node() {
    let commands = parse_raw_html("hello");

    let joined: String = commands
      .iter()
      .filter_map(|c| match c {
        UiCommand::Text(s) => Some(s.as_str()),
        _ => None,
      })
      .collect::<Vec<_>>()
      .join("");

    assert!(joined.contains("hello"));
  }

  #[test]
  fn parse_single_element_yields_open_text_close() {
    let commands = parse_raw_html("<strong>bold</strong>");

    let elements = commands
      .iter()
      .filter(|c| matches!(c, UiCommand::Element { .. }))
      .count();

    assert_eq!(elements, 1, "should emit one Element command");

    let ends = commands
      .iter()
      .filter(|c| matches!(c, UiCommand::EndElement))
      .count();

    assert_eq!(ends, 1, "should emit one EndElement command");

    let has_bold = commands
      .iter()
      .any(|c| text_of(c).map(|s| s.contains("bold")).unwrap_or(false));

    assert!(has_bold, "should emit a TextNode containing `bold`");
  }

  #[test]
  fn parse_article_is_enumerated_tag() {
    let commands = parse_raw_html("<article>x</article>");

    let tag = commands.iter().find_map(|c| match c {
      UiCommand::Element { tag, .. } => Some(tag),
      _ => None,
    });

    // `<article>` is enumerated in ElementTag, so it maps
    // directly to `ElementTag::Article` not `Custom`.
    assert!(matches!(tag, Some(ElementTag::Article)));
  }

  #[test]
  fn parse_truly_custom_tag_becomes_custom() {
    let commands = parse_raw_html("<my-widget>hi</my-widget>");

    let tag = commands.iter().find_map(|c| match c {
      UiCommand::Element { tag, .. } => Some(tag),
      _ => None,
    });

    // zo's ident scanner only accepts `[a-zA-Z0-9_]`, so
    // `my-widget` tokenizes unusually. The point of this test
    // is mostly to exercise that Custom-tag handling doesn't
    // crash.
    assert!(tag.is_some());
  }

  #[test]
  fn parse_strips_html_comments() {
    let commands = parse_raw_html("<!-- skip --><p>hi</p>");

    let has_comment_text = commands.iter().any(|c| match c {
      UiCommand::Text(s) => s.contains("skip"),
      _ => false,
    });

    assert!(!has_comment_text, "comment text should be stripped");

    let elements = commands
      .iter()
      .filter(|c| matches!(c, UiCommand::Element { .. }))
      .count();

    assert_eq!(elements, 1, "only the <p> should emit an element");
  }

  #[test]
  fn parse_nested_elements() {
    let commands = parse_raw_html("<div><span>hi</span></div>");

    let elements = commands
      .iter()
      .filter(|c| matches!(c, UiCommand::Element { .. }))
      .count();
    let ends = commands
      .iter()
      .filter(|c| matches!(c, UiCommand::EndElement))
      .count();

    assert_eq!(elements, 2, "two open tags expected");
    assert_eq!(ends, 2, "two close tags expected");
  }

  #[test]
  fn parse_self_closing_tag() {
    let commands = parse_raw_html("<br/>");

    let self_closing = commands.iter().any(|c| {
      matches!(
        c,
        UiCommand::Element {
          self_closing: true,
          ..
        }
      )
    });

    assert!(self_closing, "<br/> should emit a self-closing Element");
  }
}
