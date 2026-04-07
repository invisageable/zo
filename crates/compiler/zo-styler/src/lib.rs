/// Shorthand-to-CSS property mapping table.
///
/// Tailwind-inspired short names that expand to full CSS
/// properties. Stored as a static lookup — no allocation.
const PROPERTIES: &[(&str, &str)] = &[
  // dimensions.
  ("w", "width"),
  ("h", "height"),
  ("mw", "max-width"),
  ("mh", "max-height"),
  ("mnw", "min-width"),
  ("mnh", "min-height"),
  // spacing.
  ("m", "margin"),
  ("mt", "margin-top"),
  ("mr", "margin-right"),
  ("mb", "margin-bottom"),
  ("ml", "margin-left"),
  ("mx", "margin-inline"),
  ("my", "margin-block"),
  ("p", "padding"),
  ("pt", "padding-top"),
  ("pr", "padding-right"),
  ("pb", "padding-bottom"),
  ("pl", "padding-left"),
  ("px", "padding-inline"),
  ("py", "padding-block"),
  // typography.
  ("fs", "font-size"),
  ("fw", "font-weight"),
  ("ff", "font-family"),
  ("lh", "line-height"),
  ("ls", "letter-spacing"),
  ("ta", "text-align"),
  ("td", "text-decoration"),
  ("tt", "text-transform"),
  // display and layout.
  ("d", "display"),
  ("pos", "position"),
  ("t", "top"),
  ("r", "right"),
  ("b", "bottom"),
  ("l", "left"),
  ("z", "z-index"),
  ("ov", "overflow"),
  ("fl", "flex"),
  ("fd", "flex-direction"),
  ("fwr", "flex-wrap"),
  ("ai", "align-items"),
  ("jc", "justify-content"),
  ("g", "gap"),
  // visual.
  ("bg", "background"),
  ("c", "color"),
  ("op", "opacity"),
  ("br", "border-radius"),
  ("bd", "border"),
  ("bs", "box-shadow"),
  ("cur", "cursor"),
  ("tr", "transition"),
];

/// Resolves a shorthand property name to its full CSS name.
///
/// If the name is found in the dictionary, the full CSS name is
/// returned. Otherwise the input is returned verbatim — this
/// handles full CSS names and custom properties (`--foo`).
#[inline]
pub fn resolve(name: &str) -> &str {
  for &(short, full) in PROPERTIES {
    if short == name {
      return full;
    }
  }

  name
}

// --- Style rule types ---

/// A single CSS property declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleProp {
  /// Property name (short or full, pre-resolution).
  pub name: String,
  /// Raw CSS value string.
  pub value: String,
}

/// A CSS rule: selector + declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
  /// Selector string: `"p"`, `".btn"`, `"html body"`.
  pub selector: String,
  /// Property declarations.
  pub props: Vec<StyleProp>,
}

/// A complete stylesheet block.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleSheet {
  pub rules: Vec<StyleRule>,
  pub scope_hash: Option<String>,
}

/// Generates a scope hash from a source byte slice.
///
/// Uses FNV-1a for speed (the hash only needs to be unique
/// within a single compilation unit, not cryptographic).
pub fn scope_hash(source: &[u8]) -> String {
  let mut h: u32 = 0x811c_9dc5;

  for &b in source {
    h ^= b as u32;
    h = h.wrapping_mul(0x0100_0193);
  }

  format!("_zo_{:x}", h & 0xFFFF)
}

/// Compiles a [`StyleSheet`] into a CSS string.
///
/// Resolves shorthand property names, applies the optional
/// scope hash to selectors.
pub fn compile(sheet: &StyleSheet) -> String {
  let mut css = String::new();

  for rule in &sheet.rules {
    emit_rule(&mut css, rule, sheet.scope_hash.as_deref());
  }

  css
}

fn emit_rule(css: &mut String, rule: &StyleRule, scope_hash: Option<&str>) {
  css.push_str(&rule.selector);

  if let Some(hash) = scope_hash {
    // Append scope class to each simple selector.
    // e.g. `p` -> `p._zo_a3f2`
    css.push('.');
    css.push_str(hash);
  }

  css.push_str(" {\n");

  for prop in &rule.props {
    let full_name = resolve(&prop.name);

    css.push_str("  ");
    css.push_str(full_name);
    css.push_str(": ");
    css.push_str(&prop.value);
    css.push_str(";\n");
  }

  css.push_str("}\n");
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_known_shorthands() {
    assert_eq!(resolve("w"), "width");
    assert_eq!(resolve("h"), "height");
    assert_eq!(resolve("m"), "margin");
    assert_eq!(resolve("p"), "padding");
    assert_eq!(resolve("fs"), "font-size");
    assert_eq!(resolve("fw"), "font-weight");
    assert_eq!(resolve("ta"), "text-align");
    assert_eq!(resolve("d"), "display");
    assert_eq!(resolve("bg"), "background");
    assert_eq!(resolve("c"), "color");
    assert_eq!(resolve("br"), "border-radius");
    assert_eq!(resolve("jc"), "justify-content");
    assert_eq!(resolve("ai"), "align-items");
    assert_eq!(resolve("g"), "gap");
    assert_eq!(resolve("cur"), "cursor");
    assert_eq!(resolve("tr"), "transition");
  }

  #[test]
  fn resolve_full_css_names_passthrough() {
    assert_eq!(resolve("color"), "color");
    assert_eq!(resolve("width"), "width");
    assert_eq!(resolve("font-size"), "font-size");
    assert_eq!(resolve("justify-content"), "justify-content");
  }

  #[test]
  fn resolve_custom_properties_passthrough() {
    assert_eq!(resolve("--primary"), "--primary");
    assert_eq!(resolve("--spacing-lg"), "--spacing-lg");
  }

  #[test]
  fn resolve_unknown_passthrough() {
    assert_eq!(resolve("foo"), "foo");
    assert_eq!(resolve(""), "");
  }

  #[test]
  fn compile_simple_rule() {
    let sheet = StyleSheet {
      rules: vec![StyleRule {
        selector: "p".into(),
        props: vec![
          StyleProp {
            name: "color".into(),
            value: "cyan".into(),
          },
          StyleProp {
            name: "fw".into(),
            value: "800".into(),
          },
        ],
      }],
      scope_hash: None,
    };

    let css = compile(&sheet);

    assert_eq!(css, "p {\n  color: cyan;\n  font-weight: 800;\n}\n");
  }

  #[test]
  fn compile_with_scope_hash() {
    let sheet = StyleSheet {
      rules: vec![StyleRule {
        selector: ".title".into(),
        props: vec![StyleProp {
          name: "ta".into(),
          value: "center".into(),
        }],
      }],
      scope_hash: Some("_zo_a3f2".into()),
    };

    let css = compile(&sheet);

    assert_eq!(css, ".title._zo_a3f2 {\n  text-align: center;\n}\n");
  }

  #[test]
  fn compile_multiple_rules() {
    let sheet = StyleSheet {
      rules: vec![
        StyleRule {
          selector: ".title".into(),
          props: vec![StyleProp {
            name: "c".into(),
            value: "cyan".into(),
          }],
        },
        StyleRule {
          selector: ".subtitle".into(),
          props: vec![StyleProp {
            name: "fs".into(),
            value: "14px".into(),
          }],
        },
      ],
      scope_hash: None,
    };

    let css = compile(&sheet);

    assert_eq!(
      css,
      ".title {\n  color: cyan;\n}\n\
       .subtitle {\n  font-size: 14px;\n}\n"
    );
  }

  #[test]
  fn scope_hash_deterministic() {
    let h1 = scope_hash(b"p { color: cyan; }");
    let h2 = scope_hash(b"p { color: cyan; }");

    assert_eq!(h1, h2);
    assert!(h1.starts_with("_zo_"));
  }

  #[test]
  fn scope_hash_different_inputs() {
    let h1 = scope_hash(b"p { color: cyan; }");
    let h2 = scope_hash(b".btn { bg: red; }");

    assert_ne!(h1, h2);
  }
}
