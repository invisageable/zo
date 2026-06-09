//! The one place zo decides whether to emit ANSI color.
//!
//! clig.dev's color contract: a tool colors its output only when a
//! human is watching. Deciding it once, here, keeps every channel —
//! the diagnostics renderer, the build banner — in agreement.

use std::io::IsTerminal;

/// The output stream a color decision targets.
///
/// @note — the human channel is stderr (diagnostics, the build
/// banner); stdout carries primary output. They decide
/// independently, so a piped stdout with an interactive stderr
/// still colors the diagnostics.
#[derive(Clone, Copy, Debug)]
pub enum Stream {
  /// Standard output — primary program / machine data.
  Stdout,
  /// Standard error — the tool's messages to a human.
  Stderr,
}

impl Stream {
  /// Whether this stream is connected to a terminal.
  fn is_terminal(self) -> bool {
    match self {
      Self::Stdout => std::io::stdout().is_terminal(),
      Self::Stderr => std::io::stderr().is_terminal(),
    }
  }
}

/// The signals that decide color, read from the flag, environment,
/// and stream. Held apart from [`decide`] so the precedence is
/// unit-testable without poking process-global env or a real TTY.
struct Signals {
  /// The `--no-color` flag — the strongest "off".
  forced_off: bool,
  /// `FORCE_COLOR` is set — the escape hatch that forces color on
  /// for pipes / CI logs that do render ANSI.
  force_color: bool,
  /// `NO_COLOR` is set — the conventional "off" opt-out.
  no_color: bool,
  /// `TERM=dumb` — a terminal that can't render escapes.
  term_dumb: bool,
  /// The target stream is connected to a terminal.
  is_terminal: bool,
}

/// Resolves the signals to a color decision. Precedence, strongest
/// first: `--no-color` → `FORCE_COLOR` → `NO_COLOR` → `TERM=dumb`
/// → the TTY check.
fn decide(signals: Signals) -> bool {
  if signals.forced_off {
    return false;
  }

  if signals.force_color {
    return true;
  }

  if signals.no_color {
    return false;
  }

  if signals.term_dumb {
    return false;
  }

  signals.is_terminal
}

/// Whether ANSI color should be written to `stream`. `forced_off`
/// is the `--no-color` flag. Reads `FORCE_COLOR` / `NO_COLOR` /
/// `TERM` and the stream's TTY status, then applies [`decide`].
pub fn enabled(stream: Stream, forced_off: bool) -> bool {
  decide(Signals {
    forced_off,
    force_color: env_present("FORCE_COLOR"),
    no_color: env_present("NO_COLOR"),
    term_dumb: std::env::var_os("TERM").is_some_and(|term| term == "dumb"),
    is_terminal: stream.is_terminal(),
  })
}

/// Whether `name` holds a non-empty value. The `NO_COLOR` /
/// `FORCE_COLOR` conventions treat an empty value as unset.
fn env_present(name: &str) -> bool {
  std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
  use super::{Signals, decide};

  /// Builds signals with everything off and a non-terminal stream —
  /// the baseline a test then flips one field of.
  fn off() -> Signals {
    Signals {
      forced_off: false,
      force_color: false,
      no_color: false,
      term_dumb: false,
      is_terminal: false,
    }
  }

  /// A bare interactive terminal colors; a piped one does not.
  #[test]
  fn tty_is_the_default_gate() {
    assert!(decide(Signals {
      is_terminal: true,
      ..off()
    }));
    assert!(!decide(off()));
  }

  /// `--no-color` wins over every other signal, including
  /// `FORCE_COLOR` and an interactive terminal.
  #[test]
  fn forced_off_beats_everything() {
    assert!(!decide(Signals {
      forced_off: true,
      force_color: true,
      is_terminal: true,
      ..off()
    }));
  }

  /// `FORCE_COLOR` turns color on even when piped, and overrides
  /// `NO_COLOR` and `TERM=dumb`.
  #[test]
  fn force_color_overrides_off_signals() {
    assert!(decide(Signals {
      force_color: true,
      no_color: true,
      term_dumb: true,
      is_terminal: false,
      ..off()
    }));
  }

  /// `NO_COLOR` suppresses color on an interactive terminal.
  #[test]
  fn no_color_suppresses_on_tty() {
    assert!(!decide(Signals {
      no_color: true,
      is_terminal: true,
      ..off()
    }));
  }

  /// `TERM=dumb` suppresses color on an interactive terminal.
  #[test]
  fn term_dumb_suppresses_on_tty() {
    assert!(!decide(Signals {
      term_dumb: true,
      is_terminal: true,
      ..off()
    }));
  }
}
