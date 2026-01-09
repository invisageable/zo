//! Demonstrates eazy-tweener: Tween, Timeline, and Stagger.
//!
//! Shows 5 balls with staggered bounce animations using the new
//! GSAP-like animation runtime built on eazy.

use eazy::Controllable;
use eazy::Curve;
use eazy::Easing;
use eazy::Stagger;
use eazy::StaggerFrom;
use eazy::Timeline;
use eazy::Tween;

use eframe::egui;

use std::time::Instant;

const BALL_COUNT: usize = 5;
const BALL_RADIUS: f32 = 15.0;
const BALL_SPACING: f32 = 50.0;

pub struct TweenerApp {
  last_frame: Instant,
  timeline: Timeline,
  ball_positions: [f32; BALL_COUNT],
}

impl Default for TweenerApp {
  fn default() -> Self {
    // Create staggered bounce animations for each ball.
    let tweens = (0..BALL_COUNT)
      .map(|_| {
        Tween::to(0.0_f32, 1.0)
          .duration(1.0)
          .easing(Easing::OutBounce)
          .build()
      })
      .collect::<Vec<_>>();

    // Build timeline with staggered start times from center outward.
    let mut timeline = Timeline::builder()
      .push_staggered(tweens, Stagger::each(0.15).from(StaggerFrom::Center))
      .build();

    timeline.play();

    Self {
      last_frame: Instant::now(),
      timeline,
      ball_positions: [0.0; BALL_COUNT],
    }
  }
}

impl TweenerApp {
  fn restart(&mut self) {
    // Rebuild the timeline.
    let tweens = (0..BALL_COUNT)
      .map(|_| {
        Tween::to(0.0_f32, 1.0)
          .duration(1.0)
          .easing(Easing::OutBounce)
          .build()
      })
      .collect::<Vec<_>>();

    self.timeline = Timeline::builder()
      .push_staggered(tweens, Stagger::each(0.15).from(StaggerFrom::Center))
      .build();

    self.timeline.play();
    self.ball_positions = [0.0; BALL_COUNT];
  }
}

impl eframe::App for TweenerApp {
  fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
    egui::Color32::from_rgb(30, 30, 40).to_normalized_gamma_f32()
  }

  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // Calculate delta time.
    let now = Instant::now();
    let delta = now.duration_since(self.last_frame).as_secs_f32();

    self.last_frame = now;

    // Tick the timeline.
    self.timeline.tick(delta);

    // Update ball positions from timeline progress.
    // Note: In a real app, you'd store tween references or use a different
    // approach. For this demo, we approximate based on timeline progress.
    let progress = self.timeline.progress();

    for (i, pos) in self.ball_positions.iter_mut().enumerate() {
      // Each ball's progress is offset by stagger.
      let stagger_offset = 0.15 * i as f32;
      let total_duration = 1.0 + 0.15 * (BALL_COUNT - 1) as f32;
      let ball_progress =
        ((progress * total_duration - stagger_offset) / 1.0).clamp(0.0, 1.0);

      // Apply OutBounce easing.
      *pos = Easing::OutBounce.y(ball_progress);
    }

    egui::CentralPanel::default().show(ctx, |ui| {
      ui.vertical_centered(|ui| {
        ui.heading("eazy-tweener Demo");
        ui.label("Staggered bounce animation from center outward");
        ui.add_space(10.0);

        if ui.button("Restart").clicked() {
          self.restart();
        }

        ui.add_space(10.0);

        ui.label(format!(
          "Timeline: {:.1}% | State: {:?}",
          self.timeline.progress() * 100.0,
          self.timeline.state()
        ));

        ui.add_space(20.0);

        // Draw the animation area.
        let (w, h) = (BALL_SPACING * (BALL_COUNT + 1) as f32, 200.0);
        let (rect, _) =
          ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Background.
        painter.rect_filled(rect, 8.0, egui::Color32::from_rgb(20, 20, 30));

        // Draw balls.
        let colors = [
          egui::Color32::from_rgb(255, 100, 100),
          egui::Color32::from_rgb(255, 200, 100),
          egui::Color32::from_rgb(100, 255, 100),
          egui::Color32::from_rgb(100, 200, 255),
          egui::Color32::from_rgb(200, 100, 255),
        ];

        for (i, &progress) in self.ball_positions.iter().enumerate() {
          let x = rect.min.x + BALL_SPACING * (i + 1) as f32;
          let y_range = h - BALL_RADIUS * 3.0;
          let y = rect.max.y - BALL_RADIUS - progress * y_range;

          painter.circle_filled(
            egui::pos2(x, y),
            BALL_RADIUS,
            colors[i % colors.len()],
          );
        }
      });
    });

    // Keep animating.
    ctx.request_repaint();
  }
}

fn main() -> eframe::Result<()> {
  eframe::run_native(
    "egui-tweener-timeline-stagger",
    eframe::NativeOptions::default(),
    Box::new(|_cc| Ok(Box::new(TweenerApp::default()))),
  )
}
