use eazy::Curve;
use eazy::oscillatory::bounce::OutBounce;

use eframe::egui;

use std::time::{Duration, Instant};

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

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let elapsed = self.start_time.elapsed().as_secs_f32();
    let t = (elapsed % self.duration) / self.duration; // Normalize to [0,1]

    let bounce_height = OutBounce.y(t); // Apply easing

    egui::CentralPanel::default().show(ctx, |ui| {
      ui.vertical_centered(|ui| {
        let (w, h) = (300.0, 300.0);
        let radius = 20.0;
        let ypos = h - bounce_height * (h - radius * 2.0);

        let painter = ui.painter();
        let center = egui::pos2(w / 2.0, ypos);

        // Background box
        painter.rect_filled(
          egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(w, h)),
          0.0,
          egui::Color32::DARK_GRAY,
        );

        // Ball
        painter.circle_filled(center, radius, egui::Color32::LIGHT_GREEN);
      });
    });

    ctx.request_repaint(); // Keep animating
  }
}

fn main() -> eframe::Result<()> {
  let native_options = eframe::NativeOptions {
    // window_size: Some(egui::vec2(400.0, 400.0)),
    ..Default::default()
  };

  eframe::run_native(
    "egui-bouncing-ball-animation",
    native_options,
    Box::new(|_cc| Ok(Box::new(BounceApp::default()))),
  )
}
