//! User-agent stylesheet — the default look of unstyled HTML.
//!
//! @see [`html.css — chrome`](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/html/resources/html.css)
//! @see [`rendering — whatwg`](https://html.spec.whatwg.org/multipage/rendering.html)
//!
//! Values transcribed from the WHATWG HTML rendering spec and
//! WebKit's `html.css`. Frozen at v1; extend as new tags become
//! relevant. Lookup is keyed by the canonical lowercase tag name
//! (`ElementTag::as_str()`), so `Custom("foo")` works too.

use super::computed::{Edges, FontFamily, Rgba, StylePatch, TextDecoration};

/// Convenience builder so the table below stays readable.
const fn patch() -> StylePatch {
  StylePatch::EMPTY
}

/// The user-agent sheet, evaluated before any author rule. Order inside the
/// slice is irrelevant — entries are looked up by tag.
#[rustfmt::skip]
pub const UA_SHEET: &[(&str, StylePatch)] = &[
  // root containers — small body margin matches every browser.
  ("body", StylePatch { margin: Some(Edges::all(8.0)), ..patch() }),

  // headings — sizes follow the spec's 2/1.5/1.17/1/0.83/0.67 em
  // ratios off the 16px root, with the classic vertical margins.
  ("h1", StylePatch {
    font_size: Some(32.0),
    font_weight: Some(700),
    margin: Some(Edges::v(21.44, 21.44)),
    ..patch()
  }),
  ("h2", StylePatch {
    font_size: Some(24.0),
    font_weight: Some(700),
    margin: Some(Edges::v(19.92, 19.92)),
    ..patch()
  }),
  ("h3", StylePatch {
    font_size: Some(18.72),
    font_weight: Some(700),
    margin: Some(Edges::v(18.72, 18.72)),
    ..patch()
  }),
  ("h4", StylePatch {
    font_size: Some(16.0),
    font_weight: Some(700),
    margin: Some(Edges::v(21.28, 21.28)),
    ..patch()
  }),
  ("h5", StylePatch {
    font_size: Some(13.28),
    font_weight: Some(700),
    margin: Some(Edges::v(22.18, 22.18)),
    ..patch()
  }),
  ("h6", StylePatch {
    font_size: Some(10.72),
    font_weight: Some(700),
    margin: Some(Edges::v(24.98, 24.98)),
    ..patch()
  }),

  // paragraph + inline runs.
  ("p", StylePatch {
    margin: Some(Edges::v(16.0, 16.0)),
    ..patch()
  }),
  ("strong", StylePatch { font_weight: Some(700), ..patch() }),
  ("b",      StylePatch { font_weight: Some(700), ..patch() }),

  // links — blue + underline like every browser.
  ("a", StylePatch {
    color: Some(Rgba::LINK_BLUE),
    text_decoration: Some(TextDecoration::Underline),
    ..patch()
  }),

  // monospace family for code/pre.
  ("code", StylePatch { font_family: Some(FontFamily::Mono), ..patch() }),
  ("pre",  StylePatch {
    font_family: Some(FontFamily::Mono),
    margin: Some(Edges::v(16.0, 16.0)),
    ..patch()
  }),

  // lists — left padding gives the bullet column.
  ("ul", StylePatch {
    margin: Some(Edges::v(16.0, 16.0)),
    padding: Some(Edges {
      top: 0.0,
      right: 0.0,
      bottom: 0.0,
      left: 40.0,
    }),
    ..patch()
  }),
  ("ol", StylePatch {
    margin: Some(Edges::v(16.0, 16.0)),
    padding: Some(Edges {
      top: 0.0,
      right: 0.0,
      bottom: 0.0,
      left: 40.0,
    }),
    ..patch()
  }),

  // blockquote — chrome's default ~40px side indent.
  ("blockquote", StylePatch {
    margin: Some(Edges {
      top: 16.0,
      right: 40.0,
      bottom: 16.0,
      left: 40.0,
    }),
    ..patch()
  }),
];

/// Look up the UA patch for a tag name. Returns `None` for tags
/// the sheet does not cover; the cascade then falls through to the
/// root defaults.
pub fn lookup(tag: &str) -> Option<&'static StylePatch> {
  UA_SHEET
    .iter()
    .find_map(|(name, patch)| if *name == tag { Some(patch) } else { None })
}
