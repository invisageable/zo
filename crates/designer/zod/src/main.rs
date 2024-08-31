use gpui::*;
use gpui::{div, prelude::FluentBuilder as _, RenderOnce};

#[derive(Clone, Copy, Default)]
pub struct Style;

impl Style {
  fn new() -> StyleRefinement {
    StyleRefinement {
      display: Some(Display::Flex),
      background: Some(Fill::Color(rgb(0x2e7d32).into())),
      justify_content: Some(JustifyContent::Center),
      align_items: Some(AlignItems::Center),
      size: SizeRefinement {
        width: Some(relative(1.0f32).into()),
        height: Some(relative(1.0f32).into()),
      },
      text: Some(TextStyleRefinement {
        color: Some(rgb(0xffffff).into()),
        font_size: Some(AbsoluteLength::Rems(rems(1.25f32).into())),
        ..Default::default()
      }),
      ..Default::default()
    }
  }
}

#[derive(Clone, IntoElement)]
pub struct Node {
  text: SharedString,
}

impl Node {
  pub fn new(text: &str) -> Self {
    Self {
      text: format!("{}", text).into(),
    }
  }
}

impl RenderOnce for Node {
  fn render(self, _cx: &mut WindowContext) -> impl IntoElement {
    let mut styling = Style::new();
    let mut div = div();
    let style = div.style();

    style.refine(&mut styling);
    div.children(vec![format!("hello, {}!", self.text)])
  }
}

pub enum NodeKind {
  Elmt(zo_ast::ast::Elmt),
}

struct View {
  node: Node,
}

impl Render for View {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    self.node.to_owned().render(cx)
  }
}

fn main() {
  App::new().run(|cx: &mut AppContext| {
    cx.open_window(WindowOptions::default(), |cx| {
      cx.new_view(|_cx| View {
        node: Node::new("ivs"),
      })
    })
    .unwrap();
  });
}
