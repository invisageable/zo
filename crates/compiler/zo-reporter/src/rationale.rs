//! Rationale channel — compiler-decision notes gated by
//! `--explain-decisions`.
//!
//! Standalone "severity = Note" diagnostics emitted by passes
//! that surprise the user: "this function was eliminated
//! because no path reaches it from main", "this match arm
//! is shadowed by earlier arms covering the same case", etc.
//!
//! ## Cost when disabled
//!
//! `report_rationale` short-circuits to a no-op via a single
//! relaxed atomic load. The hot path (10M LoC/s budget)
//! stays untouched — no allocation, no error materialised,
//! no further work. The atomic stays cold in cache for
//! default builds.
//!
//! ## Lifecycle
//!
//! 1. Driver parses `--explain-decisions` and calls
//!    [`enable_rationale`] before any pass runs.
//! 2. A pass deciding to eliminate / shadow / rewrite calls
//!    [`report_rationale`] with the appropriate
//!    `ErrorKind` + span.
//! 3. The Error lands in the same thread-local collector
//!    as regular errors; the severity table classifies
//!    rationale-kinds as [`Severity::Note`].
//! 4. The aggregator buckets by phase; the JSON renderer
//!    emits `severity: "note"`; the human renderer renders
//!    as ariadne `ReportKind::Advice`.

use std::sync::atomic::{AtomicBool, Ordering};

use zo_error::Error;

use crate::collector::report_error;

/// `--explain-decisions` toggle. Single source of truth for
/// the whole process — driver flips it once at startup,
/// every pass reads it via the cheap relaxed atomic load
/// in [`report_rationale`].
static RATIONALE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables (or disables) rationale emission process-wide.
/// Driver binds from `--explain-decisions`; library callers
/// (fret build pipeline, integration tests) leave it `false`.
#[inline]
pub fn enable_rationale(enabled: bool) {
  RATIONALE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// `true` when the rationale channel is open. Passes that
/// want to skip expensive work *unrelated* to the rationale
/// itself (e.g. gathering a span chain) can short-circuit
/// on this. The bare `report_rationale` call already
/// no-ops cheaply when disabled — most callers don't need
/// this helper.
#[inline]
pub fn is_rationale_enabled() -> bool {
  RATIONALE_ENABLED.load(Ordering::Relaxed)
}

/// Reports a compiler-decision rationale. No-op (returns
/// `false`) when `--explain-decisions` is off. When on,
/// the `Error` flows through the regular thread-local
/// collector and arrives at the aggregator with severity
/// `Note`.
///
/// Callers must pass an `ErrorKind` whose severity is
/// `Note` (see `zo_error::severity`); the type system
/// can't enforce this since `Error::new` is generic over
/// kind, but passing a non-Note kind would surface as a
/// hard error in the build output, which the test suite
/// will catch loudly.
#[inline]
pub fn report_rationale(error: Error) -> bool {
  if !is_rationale_enabled() {
    return false;
  }

  report_error(error)
}
