//! `--watch` mode for `zo run` / `zo build`.
//!
//! Re-runs the same one-shot handler on `.zo` file changes.
//! TTY → alt-screen + cursor home between runs (frame
//! refresh, no white flash, scrollback intact). Pipe →
//! `── rebuild ──` separator (no escape codes; safe under
//! `... | tee`).
//!
//! The watcher is registered before the first run so a
//! save during compilation queues an event for the next
//! tick instead of being lost. 20 ms debounce coalesces
//! the 2-3 fs events most editors emit per save without
//! adding perceptible latency on top of zo's sub-100 ms
//! compile times.

use notify_debouncer_mini::{
  DebounceEventResult, Debouncer, new_debouncer,
  notify::{RecommendedWatcher, RecursiveMode},
};

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Coalesces editor save bursts; below human latency.
const DEBOUNCE_MS: u64 = 20;

/// `CSI ? 1049 h` — switch to alt screen; spares scrollback.
const ENTER_ALT_SCREEN: &[u8] = b"\x1B[?1049h";
/// `CSI ? 1049 l` — restore the regular screen on exit.
const LEAVE_ALT_SCREEN: &[u8] = b"\x1B[?1049l";
/// Cursor home + clear-to-end; frame refresh, no flash.
const CLEAR_HOME: &[u8] = b"\x1B[H\x1B[J";
/// Pipe-mode boundary; no escapes so `... | tee` stays clean.
const REBUILD_SEPARATOR: &[u8] = "\n── rebuild ──\n\n".as_bytes();

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderMode {
  AltScreen,
  Separator,
}

fn detect_render_mode() -> RenderMode {
  if std::io::stdout().is_terminal() {
    RenderMode::AltScreen
  } else {
    RenderMode::Separator
  }
}

fn emit(bytes: &[u8]) {
  let mut out = std::io::stdout().lock();
  let _ = out.write_all(bytes);
  let _ = out.flush();
}

fn render_reset(mode: RenderMode) {
  match mode {
    RenderMode::AltScreen => emit(CLEAR_HOME),
    RenderMode::Separator => emit(REBUILD_SEPARATOR),
  }
}

/// Restores the regular screen on Drop. Constructed only
/// in alt-screen mode; in pipe mode no guard is created
/// and Drop's no-op branch disappears entirely.
///
/// SIGINT bypasses Drop on Rust's default handler, but
/// modern terminals reverse the `1049h` toggle when their
/// PTY child dies — visible cleanup is fine in practice
/// without pulling in a `ctrlc` dep.
struct AltScreenGuard;

impl AltScreenGuard {
  fn enter() -> Self {
    emit(ENTER_ALT_SCREEN);
    Self
  }
}

impl Drop for AltScreenGuard {
  fn drop(&mut self) {
    emit(LEAVE_ALT_SCREEN);
  }
}

/// Single-file program: parent dir, recursive — catches
/// sibling `.zo` for `pack`/`load` projects authored in a
/// single directory. Directory input: the directory
/// itself, recursive — for project layouts. Always
/// canonicalised so cwd-relative inputs don't desync the
/// watcher.
fn watch_root(input: &Path) -> std::io::Result<PathBuf> {
  let canonical = input.canonicalize()?;

  if canonical.is_dir() {
    Ok(canonical)
  } else {
    canonical.parent().map(Path::to_path_buf).ok_or_else(|| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "watch root has no parent",
      )
    })
  }
}

/// Drive a one-shot handler in a watch loop. The handler
/// runs once on entry; subsequent runs are triggered by
/// debounced `.zo`-file events on the resolved root.
/// Errors during the handler are the handler's
/// responsibility — the loop never bails on them.
pub(crate) fn watch_loop(
  input: &Path,
  mut handler: impl FnMut(),
) -> std::io::Result<()> {
  let root = watch_root(input)?;
  let mode = detect_render_mode();
  let _alt_screen =
    (mode == RenderMode::AltScreen).then(AltScreenGuard::enter);

  let (tx, rx) = mpsc::channel::<DebounceEventResult>();

  let mut debouncer: Debouncer<RecommendedWatcher> =
    new_debouncer(Duration::from_millis(DEBOUNCE_MS), tx)
      .map_err(std::io::Error::other)?;

  debouncer
    .watcher()
    .watch(&root, RecursiveMode::Recursive)
    .map_err(std::io::Error::other)?;

  render_reset(mode);
  handler();

  for res in rx {
    let events = match res {
      Ok(v) => v,
      Err(e) => {
        eprintln!("watch error: {e:?}");
        continue;
      }
    };

    let zo_changed = events
      .iter()
      .any(|e| e.path.extension().is_some_and(|ext| ext == "zo"));

    if !zo_changed {
      continue;
    }

    render_reset(mode);
    handler();
  }

  Ok(())
}
