//! Native theme — the egui `Visuals` + `Spacing` preset that
//! makes a zo program render against a browser-like surface.
//!
//! Lives in the native runtime because every value here is
//! egui-specific (`Color32`, `Visuals`, `Stroke`, `Spacing`).
//! The target-agnostic UA stylesheet (per-tag typography, margin,
//! padding) lives in `zo-ui-protocol::style::ua` and is consumed
//! by the renderer at draw time. This module is the chrome
//! around it: window background, button colors, button padding —
//! everything that egui renders globally rather than per element.
//!
//! This is the lighter precursor to PLAN_STYLER's full `zo-styler`
//! crate. When PLAN_STYLER lands, the egui-specific helpers stay
//! here and the cascade machinery moves into `zo-styler`.

use zo_ui_protocol::style::{Edges, ua_lookup};

use eframe::egui;

/// Install the default zo theme onto an egui context. Idempotent
/// — call once at startup from the `eframe::CreationContext`
/// closure.
///
/// What it sets:
/// - White panel/window/extreme background, black text override.
/// - Chromium-like button chrome (`#efefef` resting, `#e5e5e5`
///   hovered, `#dcdcdc` pressed, `#767676` border).
/// - Browser-like button padding (`6px × 2px`) so all buttons get
///   the same gutter regardless of glyph width.
pub fn style_default(ctx: &egui::Context) {
  // Browser-like base theme: pure white surface, black text.
  // Built from `Visuals::light()` and then forced to white so we
  // don't inherit egui's gray panel fill. Per-tag typography
  // comes from the UA layer in `zo-ui-protocol::style` at render
  // time.
  let mut visuals = egui::Visuals::light();
  visuals.panel_fill = egui::Color32::WHITE;
  visuals.window_fill = egui::Color32::WHITE;
  visuals.extreme_bg_color = egui::Color32::WHITE;
  visuals.override_text_color = Some(egui::Color32::BLACK);

  // Browser-like button chrome. Chromium's default `<button>` is
  // `#efefef` resting, `#e5e5e5` hovered, `#dcdcdc` pressed, with
  // a light `#767676` border. Mirror those values across egui's
  // interaction states so form controls look the same on native
  // as on the web.
  let button_resting = egui::Color32::from_rgb(0xef, 0xef, 0xef);
  let button_hovered = egui::Color32::from_rgb(0xe5, 0xe5, 0xe5);
  let button_pressed = egui::Color32::from_rgb(0xdc, 0xdc, 0xdc);
  let button_border = egui::Color32::from_rgb(0x76, 0x76, 0x76);

  visuals.widgets.inactive.bg_fill = button_resting;
  visuals.widgets.inactive.weak_bg_fill = button_resting;
  visuals.widgets.inactive.bg_stroke =
    egui::Stroke::new(1.0_f32, button_border);

  visuals.widgets.hovered.bg_fill = button_hovered;
  visuals.widgets.hovered.weak_bg_fill = button_hovered;
  visuals.widgets.hovered.bg_stroke =
    egui::Stroke::new(1.0_f32, button_border);

  visuals.widgets.active.bg_fill = button_pressed;
  visuals.widgets.active.weak_bg_fill = button_pressed;
  visuals.widgets.active.bg_stroke =
    egui::Stroke::new(1.0_f32, button_border);

  ctx.set_visuals(visuals);

  // Browser-like button padding. Chromium's default `<button>`
  // has roughly `padding: 2px 6px`, which gives every label a
  // consistent gutter so short glyphs (`-`, `+`) end up visually
  // balanced. egui exposes this as a single global
  // `Spacing::button_padding` rather than a per-widget property —
  // fine for v1 since every zo button wants the same chrome.
  ctx.global_style_mut(|s| {
    s.spacing.button_padding = egui::Vec2::new(6.0, 2.0);
  });
}

/// Build the root `egui::Frame` for the page body. Reads the
/// canonical 8px gutter from `UA_SHEET["body"]` so the table
/// stays the single source of truth — no magic numbers in the
/// renderer's `App::ui` method.
pub fn body_frame() -> egui::Frame {
  let body_margin = ua_lookup("body")
    .and_then(|p| p.margin)
    .unwrap_or(Edges::ZERO);

  egui::Frame {
    fill: egui::Color32::WHITE,
    inner_margin: egui::Margin {
      left: body_margin.left as i8,
      right: body_margin.right as i8,
      top: body_margin.top as i8,
      bottom: body_margin.bottom as i8,
    },
    outer_margin: egui::Margin::ZERO,
    ..egui::Frame::default()
  }
}
