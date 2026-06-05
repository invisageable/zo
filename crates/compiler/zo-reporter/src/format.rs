//! The diagnostic output format selector.
//!
//! One renderer is chosen per compile: `Human` paints
//! ariadne snippets on stderr for a person; `Json` and `Xml`
//! stream structured diagnostics on stdout for an agent or an
//! IDE. The two machine formats are isomorphic — same fields,
//! same identity, one carried as NDJSON, the other as a single
//! well-formed XML document.

/// Schema version shared by the machine diagnostic formats
/// (JSON and XML). Bump on any incompatible shape change — a
/// removed field, a renamed field, a narrowed closed enum.
/// Adding an optional field stays at the same version;
/// consumers ignore unknown fields. The two wire formats are
/// isomorphic and version in lockstep.
pub const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;

/// Selects which renderer materialises diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiagnosticFormat {
  /// Ariadne-styled colored snippets on stderr, for a person.
  #[default]
  Human,
  /// One NDJSON object per diagnostic on stdout, for agents.
  Json,
  /// One well-formed `<diagnostics>` document on stdout, for
  /// agents that prefer XML's explicit structural boundaries.
  Xml,
}

impl DiagnosticFormat {
  /// `true` for the formats that stream structured output to
  /// stdout. Callers use this to suppress anything that would
  /// otherwise bleed into and corrupt that machine stream
  /// (the profiler summary, stray prints).
  #[inline]
  pub fn is_machine(self) -> bool {
    matches!(self, Self::Json | Self::Xml)
  }
}
