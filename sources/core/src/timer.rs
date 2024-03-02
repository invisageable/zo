pub mod time;
pub mod unit;

use time::Time;
use unit::Unit;

#[derive(Clone, Debug, Default)]
pub struct Timer {
  pub maybe_time_start: Option<Time>,
  pub maybe_time_end: Option<Time>,
}

impl Timer {
  #[inline]
  pub fn new() -> Self {
    Self::default()
  }

  #[inline]
  pub fn start(&mut self) {
    self.maybe_time_start = Some(Time::now());
  }

  #[inline]
  pub fn end(&mut self) {
    self.maybe_time_end = Some(Time::now());
  }

  pub fn sleep(&mut self, millis: u64) {
    std::thread::sleep(std::time::Duration::from_millis(millis));
  }

  #[inline]
  pub fn reset(&mut self) {
    self.maybe_time_start = None;
    self.maybe_time_end = None;
  }

  #[inline]
  pub fn duration(&self) -> Option<std::time::Duration> {
    match (&self.maybe_time_start, &self.maybe_time_end) {
      (Some(start), Some(end)) => Time::merge(start, end),
      _ => None,
    }
  }

  #[inline]
  pub fn duration_in_unit<U: Into<Unit>>(&self, unit: U) -> Option<f64> {
    self
      .duration()
      .map(|duration| duration.as_nanos() as f64 / unit.into().as_factor())
  }
}

impl Drop for Timer {
  fn drop(&mut self) {
    self.reset();
  }
}

#[cfg(test)]
mod test {
  use super::Timer;

  #[test]
  fn should_make_timer() {
    let timer = Timer::new();

    assert!(timer.maybe_time_start == None);
    assert!(timer.maybe_time_end == None);
  }
}
