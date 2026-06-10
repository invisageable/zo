//! Compiled-CSS → `StylePatch`: the single author-layer parser.
//!
//! `zo-styler` hands the runtime a clean `selector { prop: val; }`
//! string (shorthands already expanded to full CSS names, scope
//! classes appended). We translate the subset `ComputedStyle`
//! models into per-selector `StylePatch`es so every renderer folds
//! the same author layer through `cascade::resolve` — one parser,
//! one computed style, no per-target CSS path.
//!
//! Selectors are keyed by their leading tag with any scope hash
//! stripped (`p._zo_a3f2` → `p`); a class-only selector (`.card`)
//! keeps its dotted form so it keys by class. The layout cascade
//! folds the tag rule then each `.class` an element names, so class
//! styling applies on every runtime (native + web), not the web alone.

use super::computed::{
  Align, Display, Edges, FlexDirection, GlassStyle, Justify, Material, Rgba,
  Size, StylePatch,
};

/// A parsed stylesheet: per-selector rules plus the image catalog
/// their `background_image` handles index into. The catalog keeps URL
/// strings off the POD `ComputedStyle` (the manifesto's flyweight
/// pattern), so a renderer resolves `id → images[id] → asset`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedSheet {
  /// `(selector_key, patch)` rules, in source order.
  pub rules: Vec<(String, StylePatch)>,
  /// `images[id]` is the URL for a `background_image` handle.
  pub images: Vec<String>,
}

/// Parse a compiled stylesheet into rules + an image catalog. Order is
/// preserved so later rules win when merged for one tag.
pub fn parse(css: &str) -> ParsedSheet {
  let mut rules = Vec::new();
  let mut images = Vec::new();
  let mut chars = css.chars().peekable();

  while chars.peek().is_some() {
    skip_ws(&mut chars);

    let mut selector = String::new();

    while let Some(&ch) = chars.peek() {
      if ch == '{' {
        break;
      }

      selector.push(ch);
      chars.next();
    }

    let selector = selector.trim().to_string();

    if selector.is_empty() {
      break;
    }

    // Consume `{`.
    chars.next();

    let mut patch = StylePatch::EMPTY;

    loop {
      skip_ws(&mut chars);

      match chars.peek() {
        Some(&'}') => {
          chars.next();
          break;
        }
        None => break,
        _ => {}
      }

      let mut name = String::new();

      while let Some(&ch) = chars.peek() {
        if ch == ':' {
          break;
        }

        name.push(ch);
        chars.next();
      }

      // Consume `:`.
      chars.next();
      skip_ws(&mut chars);

      let mut value = String::new();

      while let Some(&ch) = chars.peek() {
        if ch == ';' || ch == '}' {
          break;
        }

        value.push(ch);
        chars.next();
      }

      if chars.peek() == Some(&';') {
        chars.next();
      }

      let name = name.trim();
      let value = value.trim();

      // `background-image: url("…")` interns its URL into the catalog
      // and stores the index — the one property whose value lives off
      // the POD style, so it cannot flow through `apply_declaration`.
      if name == "background-image" {
        if let Some(url) = parse_url(value) {
          patch.background_image = Some(intern_image(&mut images, url));
        }
      } else {
        apply_declaration(&mut patch, name, value);
      }
    }

    rules.push((selector_key(&selector), patch));
  }

  ParsedSheet { rules, images }
}

/// Fold every rule matching `tag` into one author patch (later
/// rules overwrite earlier set fields). Returns `None` when no
/// rule targets the tag, so the cascade falls through to UA + root.
pub fn author_patch(
  rules: &[(String, StylePatch)],
  tag: &str,
) -> Option<StylePatch> {
  let mut merged: Option<StylePatch> = None;

  for (selector, patch) in rules {
    if selector == tag {
      merged.get_or_insert(StylePatch::EMPTY).overlay(patch);
    }
  }

  merged
}

/// `url("x")` / `url('x')` / `url(x)` → the bare URL. Only the
/// `url(...)` form is modelled at v1; anything else yields `None`.
fn parse_url(value: &str) -> Option<String> {
  let inner = value.strip_prefix("url(")?.strip_suffix(')')?.trim();
  let unquoted = inner
    .strip_prefix('"')
    .and_then(|s| s.strip_suffix('"'))
    .or_else(|| inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
    .unwrap_or(inner)
    .trim();

  (!unquoted.is_empty()).then(|| unquoted.to_string())
}

/// Intern a URL into the catalog, returning its index. Deduplicates so
/// the same URL across rules shares one entry.
fn intern_image(images: &mut Vec<String>, url: String) -> u32 {
  if let Some(pos) = images.iter().position(|existing| *existing == url) {
    return pos as u32;
  }

  images.push(url);

  (images.len() - 1) as u32
}

/// Map one `prop: value` declaration onto the patch. Unknown
/// properties are skipped — the renderer ignores what it cannot
/// model, exactly as a browser would.
fn apply_declaration(patch: &mut StylePatch, name: &str, value: &str) {
  match name {
    "color" => patch.color = parse_color(value),
    "background" | "background-color" => patch.background = parse_color(value),
    "display" => patch.display = parse_display(value),
    "flex-direction" => patch.flex_direction = parse_flex_direction(value),
    "justify-content" => patch.justify_content = parse_justify(value),
    "align-items" => patch.align_items = parse_align(value),
    "gap" => patch.gap = parse_length(value),
    "font-size" => patch.font_size = parse_length(value),
    "font-weight" => patch.font_weight = parse_weight(value),
    "width" => patch.width = parse_size(value),
    "height" => patch.height = parse_size(value),
    "min-width" => patch.min_width = parse_size(value),
    "min-height" => patch.min_height = parse_size(value),
    "padding" => patch.padding = parse_edges(value),
    "margin" => patch.margin = parse_edges(value),
    "material" => patch.material = parse_material(value),
    _ => {}
  }
}

/// `material: glass | glass clear | solid` → `Material`. The first
/// token picks the material; a `glass` second token picks the style
/// (`clear`, else `regular`). Unknown values yield `None`, so the
/// element keeps the cascaded `Solid` default.
fn parse_material(value: &str) -> Option<Material> {
  let mut parts = value.split_whitespace();

  match parts.next()? {
    "glass" => Some(Material::Glass(match parts.next() {
      Some("clear") => GlassStyle::Clear,
      _ => GlassStyle::Regular,
    })),
    "solid" | "none" => Some(Material::Solid),
    _ => None,
  }
}

/// Key a selector for native matching, dropping any scope hash: a tag
/// selector keys by its leading tag (`p._zo_a3f2` → `p`); a class-only
/// selector keys by its first class (`.card._zo_a3f2` → `.card`).
/// Native renders one component, so the scope isolation the hash gives
/// the web collapses to the bare name here.
fn selector_key(selector: &str) -> String {
  let selector = selector.trim();

  if let Some(classes) = selector.strip_prefix('.') {
    let class = classes.split('.').next().unwrap_or(classes);

    format!(".{class}")
  } else {
    selector.split('.').next().unwrap_or(selector).to_string()
  }
}

/// Parse a CSS length (`16px`, `8`) into pixels. The unit suffix is
/// dropped; only `px` is modelled at v1.
fn parse_length(value: &str) -> Option<f32> {
  value.trim_end_matches("px").trim().parse().ok()
}

/// Parse a dimension into `Size` (`auto`, `50%`, `120px`).
fn parse_size(value: &str) -> Option<Size> {
  if value == "auto" {
    return Some(Size::Auto);
  }

  if let Some(percent) = value.strip_suffix('%') {
    return percent.trim().parse().ok().map(Size::Percent);
  }

  parse_length(value).map(Size::Px)
}

/// Parse a 1-to-4 value edge shorthand (`8px`, `16px 0`,
/// `1px 2px 3px 4px`) in CSS top/right/bottom/left order.
fn parse_edges(value: &str) -> Option<Edges> {
  let parts: Vec<f32> = value
    .split_whitespace()
    .filter_map(|p| p.trim_end_matches("px").parse().ok())
    .collect();

  match parts.as_slice() {
    [all] => Some(Edges::all(*all)),
    [vertical, horizontal] => Some(Edges {
      top: *vertical,
      right: *horizontal,
      bottom: *vertical,
      left: *horizontal,
    }),
    [top, right, bottom, left] => Some(Edges {
      top: *top,
      right: *right,
      bottom: *bottom,
      left: *left,
    }),
    _ => None,
  }
}

/// `font-weight: 700 | bold | normal` → numeric weight.
fn parse_weight(value: &str) -> Option<u16> {
  match value {
    "bold" => Some(700),
    "normal" => Some(400),
    _ => value.parse().ok(),
  }
}

fn parse_display(value: &str) -> Option<Display> {
  Some(match value {
    "block" => Display::Block,
    "inline" => Display::Inline,
    "flex" => Display::Flex,
    "grid" => Display::Grid,
    "none" => Display::None,
    _ => return None,
  })
}

fn parse_flex_direction(value: &str) -> Option<FlexDirection> {
  Some(match value {
    "row" => FlexDirection::Row,
    "column" => FlexDirection::Column,
    _ => return None,
  })
}

fn parse_justify(value: &str) -> Option<Justify> {
  Some(match value {
    "start" | "flex-start" => Justify::Start,
    "center" => Justify::Center,
    "end" | "flex-end" => Justify::End,
    "space-between" => Justify::SpaceBetween,
    _ => return None,
  })
}

fn parse_align(value: &str) -> Option<Align> {
  Some(match value {
    "stretch" => Align::Stretch,
    "start" | "flex-start" => Align::Start,
    "center" => Align::Center,
    "end" | "flex-end" => Align::End,
    _ => return None,
  })
}

/// Parse a CSS color into target-agnostic `Rgba`. Named subset +
/// 3/6-digit hex; the same surface the egui path used before.
pub fn parse_color(value: &str) -> Option<Rgba> {
  if let Some(hex) = value.strip_prefix('#') {
    return parse_hex(hex);
  }

  Some(match value {
    "black" => Rgba::BLACK,
    "white" => Rgba::WHITE,
    "red" => Rgba::rgb(255, 0, 0),
    "green" => Rgba::rgb(0, 128, 0),
    "blue" => Rgba::rgb(0, 0, 255),
    "cyan" => Rgba::rgb(0, 255, 255),
    "magenta" => Rgba::rgb(255, 0, 255),
    "yellow" => Rgba::rgb(255, 255, 0),
    "orange" => Rgba::rgb(255, 165, 0),
    "gray" | "grey" => Rgba::rgb(128, 128, 128),
    "transparent" => Rgba::TRANSPARENT,
    _ => return None,
  })
}

fn parse_hex(hex: &str) -> Option<Rgba> {
  let bytes = hex.as_bytes();

  match bytes.len() {
    3 => Some(Rgba::rgb(
      hex_digit(bytes[0])? * 17,
      hex_digit(bytes[1])? * 17,
      hex_digit(bytes[2])? * 17,
    )),
    6 => Some(Rgba::rgb(
      hex_byte(bytes[0], bytes[1])?,
      hex_byte(bytes[2], bytes[3])?,
      hex_byte(bytes[4], bytes[5])?,
    )),
    _ => None,
  }
}

fn hex_digit(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

fn hex_byte(hi: u8, lo: u8) -> Option<u8> {
  Some(hex_digit(hi)? * 16 + hex_digit(lo)?)
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars>) {
  while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
    chars.next();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Rules-only view of `super::parse`, so the rule-focused tests
  /// stay terse; the catalog-aware tests call `super::parse` directly.
  fn parse(css: &str) -> Vec<(String, StylePatch)> {
    super::parse(css).rules
  }

  #[test]
  fn color_and_weight() {
    let rules = parse("p { color: cyan; font-weight: 800; }");
    let patch = author_patch(&rules, "p").unwrap();

    assert_eq!(patch.color, Some(Rgba::rgb(0, 255, 255)));
    assert_eq!(patch.font_weight, Some(800));
  }

  #[test]
  fn background_hex() {
    let rules = parse("button { background: #b2f5ea; }");
    let patch = author_patch(&rules, "button").unwrap();

    assert_eq!(patch.background, Some(Rgba::rgb(178, 245, 234)));
  }

  #[test]
  fn background_color_alias() {
    let rules = parse("button { background-color: black; color: white; }");
    let patch = author_patch(&rules, "button").unwrap();

    assert_eq!(patch.background, Some(Rgba::BLACK));
    assert_eq!(patch.color, Some(Rgba::WHITE));
  }

  #[test]
  fn scoped_selector_keys_by_tag() {
    let rules = parse("p._zo_a3f2 { color: blue; }");

    assert!(author_patch(&rules, "p").is_some());
  }

  #[test]
  fn scoped_class_selector_keys_by_class() {
    // A scoped `$: {}` rewrites `.card` → `.card._zo_a3f2`; native
    // must still key it `.card` so `<div class="card">` resolves.
    let rules = parse(".card._zo_a3f2 { background: red; }");

    assert_eq!(
      author_patch(&rules, ".card").unwrap().background,
      Some(Rgba::rgb(255, 0, 0))
    );
  }

  #[test]
  fn flex_layout_props() {
    let rules = parse(
      "div { display: flex; flex-direction: column; \
       justify-content: space-between; align-items: center; gap: 12px; }",
    );
    let patch = author_patch(&rules, "div").unwrap();

    assert_eq!(patch.display, Some(Display::Flex));
    assert_eq!(patch.flex_direction, Some(FlexDirection::Column));
    assert_eq!(patch.justify_content, Some(Justify::SpaceBetween));
    assert_eq!(patch.align_items, Some(Align::Center));
    assert_eq!(patch.gap, Some(12.0));
  }

  #[test]
  fn edges_shorthand() {
    assert_eq!(parse_edges("8px"), Some(Edges::all(8.0)));
    assert_eq!(parse_edges("16px 0"), Some(Edges::v(16.0, 16.0)));
    assert_eq!(
      parse_edges("1px 2px 3px 4px"),
      Some(Edges {
        top: 1.0,
        right: 2.0,
        bottom: 3.0,
        left: 4.0,
      })
    );
  }

  #[test]
  fn size_units() {
    assert_eq!(parse_size("auto"), Some(Size::Auto));
    assert_eq!(parse_size("50%"), Some(Size::Percent(50.0)));
    assert_eq!(parse_size("120px"), Some(Size::Px(120.0)));
  }

  #[test]
  fn later_rule_wins_on_merge() {
    let rules = parse("p { color: red; } p { color: blue; }");
    let patch = author_patch(&rules, "p").unwrap();

    assert_eq!(patch.color, Some(Rgba::rgb(0, 0, 255)));
  }

  #[test]
  fn unknown_tag_yields_none() {
    let rules = parse("p { color: red; }");

    assert!(author_patch(&rules, "h1").is_none());
  }

  #[test]
  fn material_glass_regular_and_clear() {
    let rules =
      parse("button { material: glass; } .card { material: glass clear; }");

    assert_eq!(
      author_patch(&rules, "button").unwrap().material,
      Some(Material::Glass(GlassStyle::Regular))
    );
    assert_eq!(
      author_patch(&rules, ".card").unwrap().material,
      Some(Material::Glass(GlassStyle::Clear))
    );
  }

  #[test]
  fn material_solid_and_unknown() {
    let rules = parse("a { material: solid; } b { material: marble; }");

    assert_eq!(
      author_patch(&rules, "a").unwrap().material,
      Some(Material::Solid)
    );
    // An unmodelled value leaves `material` unset → cascaded default.
    assert_eq!(author_patch(&rules, "b").unwrap().material, None);
  }

  #[test]
  fn material_coexists_with_background_tint() {
    let rules = parse("button { material: glass; background: #3b82f6; }");
    let patch = author_patch(&rules, "button").unwrap();

    assert_eq!(patch.material, Some(Material::Glass(GlassStyle::Regular)));
    assert_eq!(patch.background, Some(Rgba::rgb(59, 130, 246)));
  }

  #[test]
  fn background_image_interns_into_catalog() {
    let sheet = super::parse(r#"body { background-image: url("a.jpg"); }"#);

    assert_eq!(sheet.images, vec!["a.jpg".to_string()]);
    assert_eq!(
      author_patch(&sheet.rules, "body").unwrap().background_image,
      Some(0)
    );
  }

  #[test]
  fn background_image_quote_and_paren_forms() {
    let sheet = super::parse(
      "a { background-image: url('x'); } b { background-image: url(y); }",
    );

    assert_eq!(sheet.images, vec!["x".to_string(), "y".to_string()]);
    assert_eq!(
      author_patch(&sheet.rules, "a").unwrap().background_image,
      Some(0)
    );
    assert_eq!(
      author_patch(&sheet.rules, "b").unwrap().background_image,
      Some(1)
    );
  }

  #[test]
  fn background_image_dedupes_shared_url() {
    let sheet = super::parse(
      r#"a { background-image: url("p"); } b { background-image: url("p"); }"#,
    );

    assert_eq!(sheet.images, vec!["p".to_string()]);
    assert_eq!(
      author_patch(&sheet.rules, "b").unwrap().background_image,
      Some(0)
    );
  }

  #[test]
  fn background_image_malformed_yields_none() {
    let sheet = super::parse("a { background-image: none; }");

    assert!(sheet.images.is_empty());
    assert_eq!(
      author_patch(&sheet.rules, "a").unwrap().background_image,
      None
    );
  }
}
