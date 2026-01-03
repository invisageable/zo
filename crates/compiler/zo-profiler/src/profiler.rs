use crate::quotes::QUOTES;

use zo_buffer::Buffer;

use nanorand::{Rng, WyRand};
use rustc_hash::FxHashMap as HashMap;

use std::time::{Duration, Instant};

/// A profiler for tracking compilation phases.
#[derive(Debug, Clone)]
pub struct Profiler {
  /// The zimes for each phase.
  phase_times: HashMap<String, Duration>,
  /// The start times for active phases.
  active_phases: HashMap<String, Instant>,
  /// The total lines processed.
  total_lines: usize,
  /// The output file generated.
  output: String,
  /// Number of tokens processed.
  tokens_count: usize,
  /// Number of nodes parsed.
  nodes_count: usize,
  /// Number of nodes annotated.
  inferences_count: usize,
  /// Number of code artifacts generated.
  artifacts_count: usize,
  /// Number of artifacts linked.
  artifacts_linked: usize,
}
impl Profiler {
  /// Create a new profiler.
  pub fn new() -> Self {
    Self {
      phase_times: HashMap::default(),
      active_phases: HashMap::default(),
      total_lines: 0,
      output: "".into(),
      tokens_count: 0,
      nodes_count: 0,
      inferences_count: 0,
      artifacts_count: 0,
      artifacts_linked: 0,
    }
  }

  /// Adds a new start timing a phase.
  pub fn start_phase(&mut self, phase_name: &str) {
    self.active_phases.insert(phase_name.into(), Instant::now());
  }

  /// Adds a new end timing a phase.
  pub fn end_phase(&mut self, phase_name: &str) {
    if let Some(start_time) = self.active_phases.remove(phase_name) {
      let duration = start_time.elapsed();

      self.phase_times.insert(phase_name.into(), duration);
    }
  }

  /// Gets the duration for a specific phase.
  pub fn phase_time(&self, phase_name: &str) -> Option<Duration> {
    self.phase_times.get(phase_name).copied()
  }

  /// Gets total frontend time (tokenizer + parser + analyzer).
  pub fn frontend_time(&self) -> Duration {
    let tokenizer = self.phase_time("tokenizer").unwrap_or(Duration::ZERO);
    let parser = self.phase_time("parser").unwrap_or(Duration::ZERO);
    let analyzer = self.phase_time("analyzer").unwrap_or(Duration::ZERO);

    tokenizer + parser + analyzer
  }

  /// Gets total backend time (codegen phases)
  pub fn backend_time(&self) -> Duration {
    let codegen = self.phase_time("codegen").unwrap_or(Duration::ZERO);
    let linker = self.phase_time("linker").unwrap_or(Duration::ZERO);

    codegen + linker
  }

  /// Gets total compilation time.
  pub fn total_time(&self) -> Duration {
    self.phase_times.values().sum()
  }

  /// Format time duration with appropriate unit
  fn format_time(&self, duration: Duration) -> String {
    let nanos = duration.as_nanos();
    let micros = nanos as f64 / 1000.0;

    if micros < 1000.0 {
      format!("{micros:.3} μs")
    } else if micros < 1_000_000.0 {
      format!("{:.3} ms", duration.as_secs_f64() * 1000.0)
    } else {
      format!("{:.6} {}", duration.as_secs_f64(), "seconds")
    }
  }

  /// Format percentage with one decimal place
  fn format_percent(&self, percent: f64) -> String {
    format!("{percent:.1}")
  }

  /// Format speed with appropriate unit (LoC/s, K LoC/s, M LoC/s)
  fn format_speed(&self, speed: f64) -> String {
    if speed >= 1_000_000.0 {
      format!("{:.2}M", speed / 1_000_000.0)
    } else if speed >= 1000.0 {
      format!("{:.2}K", speed / 1000.0)
    } else {
      format!("{speed:.2}")
    }
  }

  fn percentage(&self, current_time: Duration, total_secs: f64) -> f64 {
    if total_secs > 0.0 {
      (current_time.as_secs_f64() / total_secs) * 100.0
    } else {
      0.0
    }
  }

  /// Sets the total number of lines processed.
  pub fn set_total_lines(&mut self, lines: usize) {
    self.total_lines = lines;
  }

  /// Adds lines to the total count.
  pub fn add_lines(&mut self, lines: usize) {
    self.total_lines += lines;
  }

  pub fn set_output(&mut self, output: String) {
    self.output = output;
  }

  /// Sets the number of tokens processed.
  pub fn set_tokens_count(&mut self, count: usize) {
    self.tokens_count = count;
  }

  /// Sets the number of tokens processed.
  pub fn set_nodes_count(&mut self, count: usize) {
    self.nodes_count = count;
  }

  /// Sets the number of symbols analyzed.
  pub fn set_inferences_count(&mut self, count: usize) {
    self.inferences_count = count;
  }

  /// Sets the number of artifacts generated.
  pub fn set_artifacts_count(&mut self, count: usize) {
    self.artifacts_count = count;
  }

  /// Sets the number of artifacts linked.
  pub fn set_artifacts_linked(&mut self, count: usize) {
    self.artifacts_linked = count;
  }

  /// Sets the number of artifacts linked.
  pub fn quote(&self) -> &str {
    let mut rng = WyRand::new();
    let index = rng.generate_range(0..QUOTES.len());

    QUOTES[index]
  }

  /// Prints the profiling summary.
  pub fn summary(&self, target_name: &str) {
    let mut buffer = Buffer::new();

    buffer.newline();
    buffer.str("[zo] lines processed (including blank lines and comments) — ");
    buffer.u32(self.total_lines as u32);
    buffer.str(".");
    buffer.newline();
    buffer.str("│");
    buffer.newline();

    let quote = self.quote();

    buffer.str("├── ");
    buffer.str(quote);
    buffer.newline();
    buffer.str("│");
    buffer.newline();

    let total_time = self.total_time();
    let total_secs = total_time.as_secs_f64();
    let frontend_time = self.frontend_time();
    let percentage = self.percentage(frontend_time, total_secs);

    buffer.str("├── ✓ [zo@front-end] time — ");
    buffer.str(&self.format_time(frontend_time));
    buffer.str(" (");
    buffer.str(&self.format_percent(percentage));
    buffer.str("%).");
    buffer.newline();

    if let Some(tokenizer_time) = self.phase_time("tokenizer") {
      let percentage = self.percentage(tokenizer_time, total_secs);

      buffer.str("│   ├── ⏺ [zo@tokenizer] time — ");
      buffer.str(&self.format_time(tokenizer_time));
      buffer.str(" (");
      buffer.str(&self.format_percent(percentage));
      buffer.str("%).");
      buffer.newline();

      if self.tokens_count > 0 {
        buffer.str("│   │   └── ⏺ processed — ");
        buffer.u32(self.tokens_count as u32);
        buffer.str(" tokens.");
        buffer.newline();
      }
    }

    if let Some(parser_time) = self.phase_time("parser") {
      let percentage = self.percentage(parser_time, total_secs);

      buffer.str("│   ├── ⏺ [zo@parser] time — ");
      buffer.str(&self.format_time(parser_time));
      buffer.str(" (");
      buffer.str(&self.format_percent(percentage));
      buffer.str("%).");
      buffer.newline();

      if self.nodes_count > 0 {
        buffer.str("│   │   └── ⏺ parsed — ");
        buffer.u32(self.nodes_count as u32);
        buffer.str(" nodes.");
        buffer.newline();
      }
    }

    if let Some(analyzer_time) = self.phase_time("analyzer") {
      let percentage = self.percentage(analyzer_time, total_secs);

      buffer.str("│   └── ⏺ [zo@analyzer] time — ");
      buffer.str(&self.format_time(analyzer_time));
      buffer.str(" (");
      buffer.str(&self.format_percent(percentage));
      buffer.str("%).");
      buffer.newline();

      if self.inferences_count > 0 {
        buffer.str("│       └── ⏺ annotated — ");
        buffer.u32(self.inferences_count as u32);
        buffer.str(" nodes.");
        buffer.newline();
      }
    }

    let backend_time = self.backend_time();
    let percentage = self.percentage(backend_time, total_secs);

    buffer.str("├── ✓ [zo@back-end] time — ");
    buffer.str(&self.format_time(backend_time));
    buffer.str(" (");
    buffer.str(&self.format_percent(percentage));
    buffer.str("%).");
    buffer.newline();

    if let Some(codegen_time) = self.phase_time("codegen") {
      let percentage = self.percentage(codegen_time, total_secs);

      buffer.str("│   ├── ⏺ [zo@codegen:");
      buffer.str(target_name);
      buffer.str("] time — ");
      buffer.str(&self.format_time(codegen_time));
      buffer.str(" (");
      buffer.str(&self.format_percent(percentage));
      buffer.str("%).");
      buffer.newline();

      if self.artifacts_count > 0 {
        buffer.str("│   │   └── ⏺ generated — ");
        buffer.u32(self.artifacts_count as u32);
        buffer.str(" artifacts.");
        buffer.newline();
      }
    }

    if let Some(linker_time) = self.phase_time("linker") {
      let percentage = self.percentage(linker_time, total_secs);

      buffer.str("│   └── ⏺ [zo@linker] time — ");
      buffer.str(&self.format_time(linker_time));
      buffer.str(" (");
      buffer.str(&self.format_percent(percentage));
      buffer.str("%).");
      buffer.newline();

      if self.artifacts_linked > 0 {
        buffer.str("│       └── ⏺ linked — ");
        buffer.u32(self.artifacts_linked as u32);
        buffer.str(" files.");
        buffer.newline();
      }
    }

    buffer.str("└── ✓ [zo@total] time — ");
    buffer.str(&self.format_time(total_time));
    buffer.str(" (100.0%).");
    buffer.newline();
    buffer.newline();

    let speed = self.total_lines as f64 / total_secs;

    buffer.str("⚡ speed: ");
    buffer.str(&self.format_speed(speed));
    buffer.str(" LoC/s.");
    buffer.newline();
    buffer.newline();

    print!("{}", String::from_utf8_lossy(&buffer.finish()));
  }
}
impl Default for Profiler {
  fn default() -> Self {
    Self::new()
  }
}
