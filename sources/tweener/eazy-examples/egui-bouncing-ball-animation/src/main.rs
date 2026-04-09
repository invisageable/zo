use eazy::{Curve, Easing};

use eframe::egui;

use std::time::Instant;

pub struct BounceApp {
  start_time: Instant,
  duration: f32,
}

impl Default for BounceApp {
  fn default() -> Self {
    Self {
      start_time: Instant::now(),
      duration: 2.0, // 2 seconds for full bounce
    }
  }
}

impl eframe::App for BounceApp {
  fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
    egui::Color32::WHITE.to_normalized_gamma_f32()
  }

  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let elapsed = self.start_time.elapsed().as_secs_f32();
    let t = (elapsed % self.duration) / self.duration;
    let bounce_height = Easing::OutBounce.y(t);

    ui.vertical_centered(|ui| {
      let (w, h) = (300.0, 300.0);
      let radius = 20.0;
      let ypos = h - bounce_height * (h - radius * 2.0);

      let painter = ui.painter();
      let center = egui::pos2(w / 2.0, ypos);

      painter.rect_filled(
        egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(w, h)),
        0.0,
        egui::Color32::DARK_GRAY,
      );

      painter.circle_filled(center, radius, egui::Color32::LIGHT_GREEN);
    });

    ui.ctx().request_repaint();
  }
}

fn main() -> eframe::Result<()> {
  eframe::run_native(
    "egui-bouncing-ball-animation",
    eframe::NativeOptions::default(),
    Box::new(|_cc| Ok(Box::new(BounceApp::default()))),
  )
}
