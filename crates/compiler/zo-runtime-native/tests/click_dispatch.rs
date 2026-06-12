//! ```sh
//! cargo test -p zo-runtime-native --test click_dispatch
//! ```
//!
//! Headless click-dispatch contract for the egui renderer: no
//! event fires without input, and a simulated click on a button
//! fires exactly its handler. Guards the tier-2 conditional
//! programs, whose buttons looked dead in manual testing while
//! probes showed spontaneous startup dispatches.

use zo_runtime_native::renderer::Renderer;
use zo_runtime_render::render::Render;
use zo_ui_protocol::{Attr, ElementTag, EventKind, PropValue, UiCommand};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn button(id: &str, label: &str, handler: &str) -> Vec<UiCommand> {
  vec![
    UiCommand::Element {
      tag: ElementTag::Button,
      attrs: vec![Attr::Prop {
        name: "data-id".into(),
        value: PropValue::Str(id.into()),
      }],
      self_closing: false,
    },
    UiCommand::Text(label.into()),
    UiCommand::EndElement,
    UiCommand::Event {
      widget_id: id.into(),
      event_kind: EventKind::Click,
      handler: handler.into(),
    },
  ]
}

fn conditional_stream() -> Vec<UiCommand> {
  let mut cmds = vec![UiCommand::Element {
    tag: ElementTag::Div,
    attrs: vec![],
    self_closing: false,
  }];

  cmds.extend(button("0", "menu", "__closure_0"));
  cmds.extend(button("1", "finish", "__closure_1"));
  cmds.push(UiCommand::Element {
    tag: ElementTag::P,
    attrs: vec![],
    self_closing: false,
  });
  cmds.push(UiCommand::Text("closed".into()));
  cmds.push(UiCommand::EndElement);
  cmds.push(UiCommand::EndElement);

  cmds
}

#[test]
fn no_events_fire_without_input() {
  let mut renderer = Renderer::new();
  let commands = conditional_stream();

  let mut harness = Harness::new_ui(move |ui| {
    renderer.render(&commands);
    renderer.render_with_ui(ui);

    let pending = renderer.take_pending_events();

    assert!(
      pending.is_empty(),
      "no input was given, yet events fired: {pending:?}"
    );
  });

  // Several frames: startup, layout settle, idle.
  for _ in 0..5 {
    harness.run();
  }
}

#[test]
fn clicking_a_button_fires_exactly_its_event() {
  use std::sync::{Arc, Mutex};

  let fired: Arc<Mutex<Vec<(u32, EventKind)>>> =
    Arc::new(Mutex::new(Vec::new()));
  let fired_inner = Arc::clone(&fired);
  let mut renderer = Renderer::new();
  let commands = conditional_stream();

  let mut harness = Harness::new_ui(move |ui| {
    renderer.render(&commands);
    renderer.render_with_ui(ui);

    let pending = renderer.take_pending_events();

    fired_inner
      .lock()
      .unwrap()
      .extend(pending.into_iter().map(|(id, kind, _)| (id, kind)));
  });

  harness.run();

  // Click the "menu" button by its accessibility label.
  harness.get_by_label("menu").click();
  harness.run();

  let events = fired.lock().unwrap().clone();

  assert_eq!(
    events,
    vec![(0, EventKind::Click)],
    "one click on `menu` fires exactly widget 0's Click"
  );
}
