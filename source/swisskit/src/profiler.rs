//! A simple [`Profiler`] to track function time execution.

use super::timer::{Timer, Unit};

use smol_str::SmolStr;

/// The representation of a profiler.
#[derive(Clone, Debug, Default)]
pub struct Profiler {
  /// A collection of profile.
  profiles: Profiles,
  /// A timer.
  timer: Timer,
}

impl Profiler {
  /// Creates a new profiler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Adds a new profile.
  pub fn add_profile(&mut self, name: impl Into<SmolStr>) -> &Self {
    self
      .duration_in_unit(Unit::S)
      .map(|time| {
        self.profiles.add_profile(Profile {
          name: name.into(),
          time,
        });

        self
      })
      .unwrap()
  }

  /// Starts profiling, a weapper of [`Timer::start`].
  #[inline(always)]
  pub fn start(&mut self) {
    self.timer.start();
  }

  /// Ends profiling, a weapper of [`Timer::end`].
  #[inline(always)]
  pub fn end(&mut self) {
    self.timer.end();
  }

  /// A weapper of [`Timer::duration`]
  #[inline]
  pub fn duration(&self) -> Option<std::time::Duration> {
    self.timer.duration()
  }

  /// A weapper of [`Timer::duration_in_unit`]
  #[inline]
  pub fn duration_in_unit(&self, unit: impl Into<Unit>) -> Option<f64> {
    self.timer.duration_in_unit(unit)
  }

  /// Gets the total profiling time.
  ///
  /// note — It is just a simple addition under the hood.
  #[inline(always)]
  pub fn total(&mut self) -> f64 {
    self.profiles.total()
  }

  /// Outputs a table with all profile passed.
  pub fn profile(&self) {
    println!();
    println!("╭-------------------------------------------╮");
    println!("│ process      | time             | stats   │");
    println!("│-------------------------------------------│");

    for profile in &self.profiles.0 {
      let time_in_percent = profile.time / self.profiles.total() * 100.0;
      let indent = " ".repeat(12 - profile.name.len());

      println!(
        " ⋮ {}{}│ {:.6} seconds | {:.2}%{space} ⋮",
        profile.name,
        indent,
        profile.time,
        time_in_percent,
        space = if time_in_percent < 10.0 { " " } else { "" },
      );
    }

    println!("│-------------------------------------------│");

    println!(
      "│ total        | {:.6} seconds | {:.2}% │",
      self.profiles.total(),
      self.profiles.total() / self.profiles.total() * 100.0,
    );

    println!("╰-------------------------------------------╯");
    println!();
  }
}

/// The representation of a profile.
#[derive(Clone, Debug, Default)]
pub struct Profile {
  /// The name of a profile.
  pub name: SmolStr,
  /// The time value of a profile.
  pub time: f64,
}

impl std::fmt::Display for Profile {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      f,
      "{{
      \"name\": \"{}\",
      \"time\": {}
     }}",
      self.name, self.time
    )
  }
}

/// The representation of a list of profile.
#[derive(Clone, Debug, Default)]
pub struct Profiles(pub Vec<Profile>);

impl Profiles {
  /// Adds a profile to the list.
  #[inline]
  pub fn add_profile(&mut self, profile: Profile) {
    self.push(profile);
  }

  /// Gets the sum of all profile duration time.
  #[inline(always)]
  pub fn total(&self) -> f64 {
    self.iter().map(|profile| profile.time).sum()
  }
}

impl std::ops::Deref for Profiles {
  type Target = Vec<Profile>;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::ops::DerefMut for Profiles {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl std::fmt::Display for Profiles {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    self
      .iter()
      .try_fold((), |_, profile| write!(f, "{profile}"))
  }
}
