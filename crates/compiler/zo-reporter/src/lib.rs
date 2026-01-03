pub mod aggregator;
pub mod collector;
pub mod render;
mod reporter;

#[cfg(test)]
mod tests;

pub use aggregator::{ErrorAggregator, Phase, PhaseErrors};
pub use collector::{clear_errors, collect_errors, error_count, report_error};
pub use render::{ErrorRenderer, RenderConfig, render_errors_to_stderr};
pub use reporter::Reporter;
