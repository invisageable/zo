pub mod aggregator;
pub mod collector;
pub mod fixes;
pub mod format;
pub mod json;
pub mod locate;
pub mod rationale;
pub mod render;
mod reporter;
pub mod xml;

#[cfg(test)]
mod tests;

pub use aggregator::{ErrorAggregator, Phase, PhaseErrors};
pub use collector::{
  Detail, TyNames, clear_errors, collect_diagnostics, collect_errors,
  error_count, report_error, report_error_with_detail,
  report_error_with_suggestion, report_error_with_types, total_count,
  warning_count,
};
pub use format::DiagnosticFormat;
pub use render::{ErrorRenderer, RenderConfig, render_errors_to_stderr};
pub use reporter::Reporter;
