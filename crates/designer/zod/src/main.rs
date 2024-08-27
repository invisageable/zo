use gpui::*;

struct View {
  text: SharedString,
}

impl Render for View {
  fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
      .flex()
      .bg(rgb(0x2e7d32))
      .size_full()
      .justify_center()
      .items_center()
      .text_xl()
      .text_color(rgb(0xffffff))
      .child(format!("Hello, {}!", &self.text))
  }
}

fn main() {
  App::new().run(|cx: &mut AppContext| {
    cx.open_window(WindowOptions::default(), |cx| {
      cx.new_view(|_cx| View {
        text: "World".into(),
      })
    })
    .unwrap();
  });
}
