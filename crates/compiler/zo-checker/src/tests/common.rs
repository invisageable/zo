//! Shared helpers for checker tests.
//!
//! The reporter is thread-local, so every test drains it after
//! driving the checker — no cross-test interference as long as each
//! test clears what it reports.

use zo_error::ErrorKind;
use zo_reporter::{Detail, collect_diagnostics};

/// The rename detail attached to the single buffered warning —
/// `None` when nothing was reported.
pub(crate) fn drained_rename() -> Option<(ErrorKind, String)> {
  let (errors, details) = collect_diagnostics();

  let rename = details.iter().find_map(|(error, detail)| match detail {
    Detail::Rename(name) => Some((error.kind(), name.to_string())),
    _ => None,
  });

  match rename {
    Some(rename) => Some(rename),
    None => errors.first().map(|error| (error.kind(), String::new())),
  }
}
