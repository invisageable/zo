mod profile;

use super::timer::unit::Unit;
use super::timer::Timer;

use profile::{Profile, Profiles};

#[derive(Clone, Debug, Default)]
pub struct Profiler {
  timer: Timer,
  profiles: Profiles,
}

impl Profiler {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn add_profile(&mut self, name: impl Into<smol_str::SmolStr>) -> &Self {
    self
      .duration_in_unit("s")
      .map(|time| {
        self.profiles.add_profile(Profile {
          name: name.into(),
          time,
        });

        self
      })
      .unwrap()
  }

  pub fn start(&mut self) -> &Self {
    self.timer.start();
    self
  }

  pub fn end(&mut self) -> &Self {
    self.timer.end();
    self
  }

  pub fn total(&mut self) -> f64 {
    self.profiles.total()
  }

  pub fn duration_in_unit(&self, unit: impl Into<Unit>) -> Option<f64> {
    self.timer.duration_in_unit(unit)
  }

  pub fn profile(&self) {
    // println!();
    println!("╭-------------------------------------------╮");
    println!("│ process      | time             | stats   │");
    println!("│-------------------------------------------│");

    for profile in &self.profiles.0 {
      let time_in_percent = profile.time / self.profiles.total() * 100.0;

      println!(
        " {} │ {:.6} seconds | {:.2}%{space} ⋮",
        profile.name,
        profile.time,
        time_in_percent,
        space = if time_in_percent < 10.0 { " " } else { "" },
      );
    }

    println!("│-------------------------------------------│");

    println!(
      "│ total        | {:.6} seconds | {:.2}% │",
      self.profiles.total(),
      self.profiles.total() / self.profiles.total() * 100.0
    );

    println!("╰-------------------------------------------╯");
    println!();
  }
}
