//! Style cascade — fold UA, author, and inline patches into a
//! single `ComputedStyle`. Same model as the CSS cascade, just
//! restricted to three layers and ordered by specificity:
//!
//! ```text
//!   1. UA sheet     (zo-ui-protocol::style::ua)
//!   2. Author sheet (PLAN_STYLER, runtime-injected)
//!   3. Inline       (`style="..."`, future)
//! ```
//!
//! Later layers overwrite earlier ones, property by property.

use super::computed::{ComputedStyle, StylePatch};
use super::ua;

/// Resolve the computed style for a single tag, given an optional
/// author patch and an optional inline patch. The UA layer is
/// looked up automatically by tag name.
pub fn resolve(
  tag: &str,
  author: Option<&StylePatch>,
  inline: Option<&StylePatch>,
) -> ComputedStyle {
  let mut out = ComputedStyle::ROOT;

  if let Some(p) = ua::lookup(tag) {
    p.apply(&mut out);
  }

  if let Some(p) = author {
    p.apply(&mut out);
  }

  if let Some(p) = inline {
    p.apply(&mut out);
  }

  out
}

#[cfg(test)]
mod tests {
  use super::super::computed::{FontFamily, Rgba};
  use super::*;

  #[test]
  fn unknown_tag_falls_back_to_root() {
    let s = resolve("unknown-tag", None, None);
    assert_eq!(s, ComputedStyle::ROOT);
  }

  #[test]
  fn ua_sheet_applies_h1_size() {
    let s = resolve("h1", None, None);
    assert_eq!(s.font_size, 32.0);
    assert_eq!(s.font_weight, 700);
  }

  #[test]
  fn ua_sheet_applies_link_color() {
    let s = resolve("a", None, None);
    assert_eq!(s.color, Rgba::LINK_BLUE);
  }

  #[test]
  fn author_overrides_ua() {
    let author = StylePatch {
      color: Some(Rgba::rgb(255, 0, 0)),
      ..StylePatch::EMPTY
    };
    let s = resolve("a", Some(&author), None);
    assert_eq!(s.color, Rgba::rgb(255, 0, 0));
  }

  #[test]
  fn inline_overrides_author() {
    let author = StylePatch {
      font_size: Some(24.0),
      ..StylePatch::EMPTY
    };
    let inline = StylePatch {
      font_size: Some(48.0),
      ..StylePatch::EMPTY
    };
    let s = resolve("p", Some(&author), Some(&inline));
    assert_eq!(s.font_size, 48.0);
  }

  #[test]
  fn code_uses_mono_family() {
    let s = resolve("code", None, None);
    assert_eq!(s.font_family, FontFamily::Mono);
  }
}
