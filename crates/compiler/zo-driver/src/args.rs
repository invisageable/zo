use zo_codegen_backend::Target;
use zo_reporter::DiagnosticFormat;

use std::path::PathBuf;

/// Represents an [`Args`] instance.
#[derive(clap::Args, Debug, Clone)]
pub struct Args {
  /// The source file(s) to process.
  #[arg(required = true)]
  pub files: Vec<PathBuf>,
  /// The compilation platform.
  #[arg(short, long, default_value = "native")]
  pub target: ArgsTarget,
  /// The intermediate representations flags (tokens, tree, sir, asm).
  #[arg(long, value_delimiter = ',')]
  pub emit: Vec<Stage>,
  /// The output path for the generated artifact. Matches
  /// rustc's `-o` — sets the explicit final-binary path.
  #[arg(short, long)]
  pub output: Option<PathBuf>,
  /// Directory every emitted file lands in (final binary,
  /// `--emit` dumps, the transient `.o`, staged runtime
  /// dylibs in a sibling `deps/`). Matches rustc's
  /// `--out-dir`. When unset, files land next to each
  /// source file.
  #[arg(long)]
  pub out_dir: Option<PathBuf>,
  /// Diagnostic output format. `human` (default) renders
  /// ariadne-styled colored snippets to stderr. `json`
  /// streams one NDJSON object per diagnostic to stdout;
  /// `xml` emits one well-formed `<diagnostics>` document
  /// to stdout. Both machine formats target agent / IDE
  /// consumers and share a frozen schema keyed by stable
  /// kebab-case `id`.
  #[arg(long, value_enum, default_value_t = Format::Human)]
  pub format: Format,
  /// Number of source lines of context to include before
  /// and after each diagnostic's span in a machine format
  /// (`--format=json` / `--format=xml`). `0` disables
  /// context. Default `2`. Ignored for the human renderer
  /// (which always shows full snippets via ariadne).
  #[arg(long, default_value_t = 2)]
  pub snippet_context: usize,
  /// Emit `severity: "note"` rationale entries explaining
  /// compiler decisions (DCE'd functions, unreachable
  /// match arms, …). Off by default to keep the hot path
  /// at the 10M LoC/s target.
  #[arg(long)]
  pub explain_decisions: bool,
  /// The number of worker threads to use. Defaults number of logical CPUs.
  #[arg(short, long, default_value_t = num_cpus::get())]
  pub workers: usize,
  /// The enable verbose output flag.
  #[arg(short, long)]
  pub verbose: bool,
  /// The compilation metrics flag.
  #[arg(short, long)]
  pub metrics: bool,
  /// Watch file changes.
  #[arg(long)]
  pub watch: bool,
}

/// The compilation platform. The friendly names — `native`,
/// `webview`, `ios` — are the primary surface; the bare triples
/// stay as advanced cross-compile aliases.
#[derive(clap::ValueEnum, Clone, Debug, Copy)]
#[clap(rename_all = "kebab-case")]
pub enum ArgsTarget {
  /// Host desktop, egui render. The default.
  #[value(name = "native")]
  Native,
  /// Host desktop app embedding a system webview.
  #[value(name = "webview")]
  Webview,
  /// iOS — the Simulator today (device deferred).
  #[value(name = "ios")]
  Ios,
  #[value(name = "arm64-apple-darwin")]
  Arm64AppleDarwin,
  #[value(name = "aarch64-pc-windows-msvc")]
  Arm64PcWindowsMsvc,
  #[value(name = "aarch64-unknown-linux-gnu")]
  Arm64UnknownLinuxGnu,
  #[value(name = "x86_64-apple-darwin")]
  X8664AppleDarwin,
  #[value(name = "x86_64-pc-windows-msvc")]
  X8664PcWindowsMsvc,
  #[value(name = "x86_64-unknown-linux-gnu")]
  X8664UnknownLinuxGnu,
  #[value(name = "wasm32-unknown-unknown")]
  Wasm32UnknownUnknown,
  #[value(name = "android", alias = "aarch64-linux-android")]
  Android,
}

impl ArgsTarget {
  /// Whether `run` drives the iOS Simulator — it produces an `.app`
  /// and boots/installs/launches it rather than executing the binary
  /// in-process.
  pub fn is_ios(self) -> bool {
    matches!(self, Self::Ios)
  }

  /// Whether templates render through a webview rather than the
  /// native egui window.
  pub fn is_webview(self) -> bool {
    matches!(self, Self::Webview)
  }
}

impl From<ArgsTarget> for Target {
  fn from(target: ArgsTarget) -> Self {
    match target {
      // `native`/`webview` are host desktop apps — same host codegen
      // target, differing only in how `run` renders.
      ArgsTarget::Native | ArgsTarget::Webview => Self::host(),
      // The user types `ios`; the runnable iOS today is the Simulator.
      ArgsTarget::Ios => Self::Arm64AppleIosSim,
      ArgsTarget::Arm64AppleDarwin => Self::Arm64AppleDarwin,
      ArgsTarget::Arm64PcWindowsMsvc => Self::Arm64PcWindowsMsvc,
      ArgsTarget::Arm64UnknownLinuxGnu => Self::Arm64UnknownLinuxGnu,
      ArgsTarget::X8664AppleDarwin => Self::X8664AppleDarwin,
      ArgsTarget::X8664PcWindowsMsvc => Self::X8664PcWindowsMsvc,
      ArgsTarget::X8664UnknownLinuxGnu => Self::X8664UnknownLinuxGnu,
      ArgsTarget::Wasm32UnknownUnknown => Self::Wasm32UnknownUnknown,
      ArgsTarget::Android => Self::Aarch64LinuxAndroid,
    }
  }
}

/// Diagnostic output shape. `Human` prints ariadne snippets
/// to stderr; `Json` streams one NDJSON object per error to
/// stdout; `Xml` emits one well-formed `<diagnostics>`
/// document to stdout. Both machine formats are for agentic
/// consumers and share one frozen, isomorphic schema.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lower")]
pub enum Format {
  #[default]
  Human,
  Json,
  Xml,
}

/// Bridges the clap-facing CLI enum to the renderer selector
/// the compiler consumes. Keeping the two separate spares the
/// low-level `zo-reporter` crate a `clap` dependency.
impl From<Format> for DiagnosticFormat {
  fn from(format: Format) -> Self {
    match format {
      Format::Human => Self::Human,
      Format::Json => Self::Json,
      Format::Xml => Self::Xml,
    }
  }
}

/// Represents the compiler [`Stage`] output.
#[derive(clap::ValueEnum, Clone, Debug, Copy)]
#[clap(rename_all = "lower")]
pub enum Stage {
  /// The collection of tokens.
  Tokens,
  /// The collection of parse tree nodes.
  Tree,
  /// The collection of sir instructions.
  Sir,
  Asm,
  /// The collection of all output stage.
  All,
}
