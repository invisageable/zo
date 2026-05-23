//! ```sh
//! cargo run --release --bin zon -- build zo-samples/tests/test_1000000_funcs.zo --target arm64-apple-darwin
//! ```

use crate::constants::{
  ANALYZER_NAME, CODEGEN_NAME, LINKER_NAME, PARSER_NAME, TOKENIZER_NAME,
};

use crate::stage::Stage;

use zo_analyzer::{Analyzer, AnalyzerConfig, SemanticResult};
use zo_codegen::codegen::Codegen;
use zo_codegen_backend::Target;
use zo_dce::Dce;
use zo_error::{Error, ErrorKind, Severity};
use zo_interner::Symbol;
use zo_module_resolver::{
  ImportedSymbols, ModuleExports, ModuleResolver, extract_exports,
  splice_generic_bodies,
};
use zo_parser::{Parser, ParsingResult};
use zo_pp::PrettyPrinter;
use zo_profiler::Profiler;
use zo_reporter::{
  ErrorAggregator, Reporter, json, rationale, render_errors_to_stderr,
  report_error,
};
use zo_session::Session;
use zo_sir::Sir;
use zo_span::Span;
use zo_token::{LiteralStoreBaseline, Token};
use zo_tokenizer::{TokenizationResult, Tokenizer};
use zo_tree::{NodeValue, Tree, TreeBaseline};
use zo_ty::Mutability;
use zo_value::ValueId;
use zo_value::{Local, LocalKind, Pubness};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Auto-detect the std lib search path so every caller of
/// `Compiler::new()` (the zo CLI driver, the fret build
/// pipeline, integration tests, …) gets `preload`/`io`/etc.
/// resolved without each one having to wire its own search
/// list. Resolution order:
///
/// 1. `ZO_CORE_PATH` env var — explicit override.
/// 2. `<exe-dir>/../lib/core` — installed layout.
/// 3. `<exe-dir>/../../crates/compiler-lib/core` — dev layout
///    (works for both `target/debug/zo` and
///    `target/debug/fret`, which sit at the same depth).
///
/// Returns an empty `Vec` if none of these resolve, in which
/// case preload silently no-ops and `showln` etc. surface
/// as `Undefined variable` — matches the old behavior so
/// callers needing a non-default layout can still pass
/// `with_search_paths`.
pub fn default_core_search_paths() -> Vec<PathBuf> {
  if let Ok(core_path) = env::var("ZO_CORE_PATH") {
    return vec![PathBuf::from(core_path)];
  }

  zo_host_paths::existing_lib_dirs(zo_host_paths::SYSTEM_PACK_ROOTS)
}

/// A lib.zo `pub pack X;` parsed once at discovery time and
/// kept on hand for the lazy-compile site. Pre-storing the
/// tokenization + parsing means each pack is read and parsed
/// exactly once across the whole compile, regardless of how
/// many places `load X` from. `loads` is also pre-scanned so
/// the lazy block walks the tree just once (during
/// `extract`).
struct PendingPack {
  path: PathBuf,
  tokenization: TokenizationResult,
  parsing: ParsingResult,
  loads: Vec<LoadRef>,
}

/// One `load` statement: a fully-qualified module path
/// (`core::io::*` → `[core, io]`) plus the span of the `load`
/// node in the source for diagnostics. Worklist entry for
/// the transitive-load closure.
pub type LoadRef = zo_span::Spanned<Vec<Symbol>>;

/// File-as-pack rule: returns the implicit pack identity
/// for a loaded module file, or `None` when the file is a
/// package manifest (`lib.zo`) or a binary entry
/// (`main.zo`).
///
/// The pack identity reflects the file's full relative
/// position under its search-path root, joined by `::`.
/// For `core/sys/info.zo` (search root `compiler-lib/core`)
/// the relative path is `sys/info.zo` and the implicit
/// pack is `sys::info` — every `pub fun` in that file
/// registers under owning pack `sys::info`, so
/// `sys::info::cpu_count()` resolves through the
/// `pack_fun_by_name` table without the call dispatcher
/// having to fall back to bare-name lookup.
///
/// Three cases:
///   1. Flat file under the search root (`core/math.zo`)
///      → `math` — relative parent is empty.
///   2. One-deep folder file
///      (`provider/raylib/rcore.zo`) → `raylib::rcore`.
///   3. Multi-deep folder file
///      (`core/graphics/misato/scene.zo`) →
///      `graphics::misato::scene`.
fn implicit_pack_for(path: &Path, search_paths: &[PathBuf]) -> Option<String> {
  let stem = path.file_stem().and_then(|s| s.to_str())?;

  if matches!(stem, "lib" | "main") {
    return None;
  }

  // Locate which search-path root contains `path` so the
  // relative remainder feeds the pack-identity join.
  // Without this anchor, a file outside the search roots
  // (the user's project source — `main.zo`) would
  // misread its filesystem-relative ancestors as pack
  // segments. Multi-root search paths
  // (`compiler-lib/core`, `compiler-lib/provider`) each
  // contribute their own anchor.
  let matching_root = search_paths.iter().find(|sp| path.starts_with(sp))?;
  let relative = path.strip_prefix(matching_root).ok()?;
  let relative_parent = relative.parent()?;

  let mut parts: Vec<String> = relative_parent
    .components()
    .filter_map(|c| c.as_os_str().to_str().map(str::to_owned))
    .collect();

  parts.push(stem.to_owned());

  Some(parts.join("::"))
}

/// Bundle of diagnostic-output toggles, bound from CLI in
/// lockstep. Driver builds one of these from its args; the
/// compiler fans the fields into the right state stores
/// (local fields, process-wide atomic) via
/// [`Compiler::configure_diagnostics`].
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticsConfig {
  /// `true` → stream NDJSON on stdout (instead of ariadne
  /// snippets on stderr).
  pub json: bool,
  /// Number of source lines of context to inline in each
  /// JSON diagnostic's `snippet`. Ignored when `json` is
  /// false. `0` disables context.
  pub snippet_context: usize,
  /// `true` → emit `severity: "note"` rationale entries
  /// explaining compiler decisions (DCE'd functions, …).
  pub explain_decisions: bool,
}

impl Default for DiagnosticsConfig {
  fn default() -> Self {
    Self {
      json: false,
      snippet_context: 2,
      explain_decisions: false,
    }
  }
}

/// Represents a [`Compiler`] instance.
pub struct Compiler {
  stats: Stats,
  profiler: Profiler,
  reporter: Reporter,
  module_resolver: ModuleResolver,
  /// The Guard against circular imports.
  compiling: HashSet<PathBuf>,
  /// Modules declared via `pub pack` in lib.zo.
  /// Populated during pack compilation, queried by `load`.
  module_table: HashMap<Symbol, ModuleExports>,
  /// When `true`, diagnostics stream as NDJSON on stdout
  /// instead of ariadne-styled snippets on stderr. Driver
  /// flips this from the `--format=json` CLI flag; library
  /// callers (fret build pipeline, tests) leave it `false`.
  emit_json: bool,
  /// Lines of source context inlined in each NDJSON
  /// diagnostic's `snippet.before` / `snippet.after`. Only
  /// consulted when `emit_json` is `true`. Defaults to the
  /// driver's `--snippet-context` flag value (2 unless
  /// overridden).
  snippet_context: usize,
}

/// Merges `other` into `into`. Used to fold the `exported`
/// scope of a loaded module into an analyzer-seed scope so
/// the consumer's analyzer can resolve names defined by the
/// loaded module.
fn fold_imports_into(into: &mut ImportedSymbols, other: &ImportedSymbols) {
  // Dedup on fold: a deep re-export chain (A re-exports B
  // re-exports C) surfaces the same public symbol along
  // multiple paths, and `extend`-then-deduplicate-at-lookup
  // wastes O(K×N) memory + time per fold (K = chain depth).
  // The collision case (two DIFFERENT modules each
  // declaring `pub fun foo`) raises
  // `DuplicatePublicName` instead of silently picking
  // first-wins or last-wins — the chosen winner would
  // depend on the user's transitive load order, which
  // breaks the second a library reshuffles its internal
  // load graph.
  // Dedup by `(name, owning_pack)` — two modules can each
  // expose `pub fun process` and stay disambiguated by
  // owning pack. Same `(name, owning_pack)` showing up
  // twice IS a genuine collision (the same module's
  // pub surface walked twice along different paths);
  // raise `DuplicatePublicName` only against that case
  // when bodies differ.
  let mut seen_funs: rustc_hash::FxHashSet<(Symbol, Option<Symbol>)> =
    into.funs.iter().map(|f| (f.name, f.owning_pack)).collect();

  for fun in &other.funs {
    let key = (fun.name, fun.owning_pack);

    if seen_funs.insert(key) {
      into.funs.push(fun.clone());
    } else if !into.funs.iter().any(|f| {
      f.name == fun.name
        && f.owning_pack == fun.owning_pack
        && f.body_start == fun.body_start
    }) {
      // Same `(name, owning_pack)`, different body —
      // genuine collision within the SAME pack.
      report_error(Error::new(ErrorKind::DuplicatePublicName, fun.span));
    }
  }

  let mut seen_vars: rustc_hash::FxHashSet<Symbol> =
    into.vars.iter().map(|v| v.name).collect();

  for var in &other.vars {
    if seen_vars.insert(var.name) {
      into.vars.push(*var);
    }
  }

  let mut seen_enums: rustc_hash::FxHashSet<Symbol> =
    into.enums.iter().map(|e| e.name).collect();

  for en in &other.enums {
    if seen_enums.insert(en.name) {
      into.enums.push(en.clone());
    }
  }

  for (sym, def) in &other.abstract_defs {
    into.abstract_defs.insert(*sym, def.clone());
  }

  // Coherence: two modules can each declare
  // `apply Eq for Point` and the importer would silently
  // overwrite one with `extend`. Raise a hard
  // `DuplicateAbstractImpl` against both defining sites
  // so the user can drop or rename one of the impls
  // before the compiled program does silent
  // wrong-dispatch. Same-source re-imports (the SAME
  // `defining_module` path) are no-ops — multiple
  // re-export paths through `pub load core::*` can
  // surface the same impl repeatedly and that's fine.
  for (key, incoming) in &other.abstract_impls {
    match into.abstract_impls.get(key) {
      Some(existing)
        if existing.defining_module == incoming.defining_module =>
      {
        // Same defining module reached twice through the
        // re-export graph; keep the first entry.
      }
      Some(existing) => {
        report_error(Error::new(
          ErrorKind::DuplicateAbstractImpl,
          incoming.defined_at,
        ));
        report_error(Error::new(
          ErrorKind::DuplicateAbstractImpl,
          existing.defined_at,
        ));
      }
      None => {
        into.abstract_impls.insert(*key, incoming.clone());
      }
    }
  }

  // `exported_generic_bodies` accumulate across the fold so
  // every transitive `apply Type<$T> { ... }` body lands in
  // the importer's pre-pass splice. `generic_bodies` (the
  // post-splice metadata) is populated per-importer right
  // before its `Analyzer` runs, so we don't merge it here.
  into
    .exported_generic_bodies
    .extend(other.exported_generic_bodies.iter().cloned());
}

/// Converts the harvested `ModuleExports` (pub items only)
/// into an `ImportedSymbols` value. Each var becomes a
/// `Local` with `Pubness::Yes` and `Mutability::No`. The
/// caller is responsible for filling `abstract_defs` from
/// the analyzer's `SemanticResult` — kept out of the
/// signature so zo-compiler never has to name a type owned
/// by zo-executor (layering stays clean: compiler talks to
/// zo-analyzer, zo-analyzer controls zo-executor).
fn module_exports_to_imports(exports: &ModuleExports) -> ImportedSymbols {
  let mut vars = Vec::with_capacity(exports.vars.len());

  for var in &exports.vars {
    vars.push(Local {
      name: var.name,
      ty_id: var.ty_id,
      value_id: var.init.unwrap_or(ValueId(0)),
      pubness: Pubness::Yes,
      mutability: Mutability::No,
      sir_value: var.init,
      local_kind: LocalKind::Variable,
    });
  }

  ImportedSymbols {
    funs: exports.funs.clone(),
    vars,
    enums: exports.enums.clone(),
    abstract_defs: HashMap::default(),
    abstract_impls: exports.abstract_impls.clone(),
    exported_generic_bodies: exports.generic_bodies.clone(),
    generic_bodies: Vec::new(),
  }
}

/// Combines a module's own pub items with the `exported`
/// scopes of every module it `pub load`s. Plain (non-pub)
/// `load`s never reach `re_exports`, so they don't
/// propagate beyond the loading module.
fn compute_exported_scope(
  own: ImportedSymbols,
  re_exports: &[Vec<Symbol>],
  table: &HashMap<Vec<Symbol>, ImportedSymbols>,
) -> ImportedSymbols {
  let mut exported = own;

  for re_path in re_exports {
    if let Some(scope) = table.get(re_path) {
      fold_imports_into(&mut exported, scope);
    }
  }

  exported
}

/// Builds a per-module analyzer seed by folding the
/// `exported` scope of every module in `own_loads` (looked
/// up in `table`), on top of the always-available `baseline`
/// (preload's exported scope). A module sees only its own
/// loads' pub surface — never their private transitive deps.
fn build_module_seed(
  baseline: &ImportedSymbols,
  own_loads: &[LoadRef],
  table: &HashMap<Vec<Symbol>, ImportedSymbols>,
) -> ImportedSymbols {
  let mut seed = baseline.clone();

  for load_ref in own_loads {
    if let Some(exported) = table.get(&load_ref.value) {
      fold_imports_into(&mut seed, exported);
    }
  }

  seed
}

/// Bundles every piece of mutable per-compile state the
/// module-loading driver threads between iterations.
///
/// Created at the start of `analyze_source` and dropped at
/// the end. Phase A of the recursive-DFS refactor (see
/// PLAN_MODULE_SYSTEM): the flat loop still drives, but
/// state lives here so Phase B can lift the per-iteration
/// body into a method and Phase C can replace the loop
/// with a recursive entry point.
///
/// Read order in the loop's body is the same as before —
/// every former stack-local has the SAME field name on
/// `DfsCtx`, so the only diff is the `ctx.` prefix.
struct DfsCtx {
  /// Cumulative `imports` seed — every loaded module sees
  /// what earlier modules exported (preload's `Option` /
  /// `Result` enums, core::int's primitive methods, etc.)
  /// and contributes its own exports back via
  /// `fold_imports_into`. Cloned per per-module analyzer
  /// construction; ownership flows into the final user
  /// analyzer at the end.
  imports: ImportedSymbols,
  /// Per-loaded-module `exported` scope. The user
  /// analyzer's seed (`user_seed`) reads ONLY from this
  /// table so transitively-loaded modules don't pollute
  /// the user file's scope.
  module_table_per_path: HashMap<Vec<Symbol>, ImportedSymbols>,
  /// `load foo::*;` where `foo/` is a directory expands
  /// into one file-load per `.zo` child. The folder path
  /// is remembered so the children's exports can be
  /// aggregated back under `["foo"]` before the user
  /// analyzer's seed is built.
  folder_aggregations: HashMap<Vec<Symbol>, Vec<Vec<Symbol>>>,
  /// Parsed modules cached across the topological-hoist
  /// rewind so the second visit doesn't re-tokenize +
  /// re-parse. Carries the post-parse `(TreeBaseline,
  /// LiteralStoreBaseline)` so cross-module splices that
  /// mutate the cached tree+literals in place can rewind
  /// to a pristine slate before each new splice.
  parse_cache: HashMap<
    PathBuf,
    (
      TokenizationResult,
      ParsingResult,
      TreeBaseline,
      LiteralStoreBaseline,
    ),
  >,
  /// Per-module SIR instruction stream — appended to as
  /// each loaded module compiles, then lifted ABOVE
  /// `main`'s SIR in `Compiler::compile` so module
  /// `FunDef`s are visible to the user's analyzer-emitted
  /// `Call`s.
  module_sir_instructions: Vec<zo_sir::Insn>,
  /// Running `ValueId` counter across the merged module
  /// SIR. Each loaded pack's SIR starts its own `%0` —
  /// `Sir::offset_value_ids` shifts each contributed
  /// block by this base so the final stream has globally
  /// unique ids.
  module_next_value_id: u32,
  /// Running `Label` counter — same pattern as
  /// `module_next_value_id` but for `Label` / `Jump` /
  /// `BranchIfNot`. `Sir::offset_labels` does the shift.
  module_next_label_id: u32,
  /// System-pack roots (`core`, `provider`, …) — loads
  /// through these bypass the user lib.zo membership
  /// check because the std layout is what governs them,
  /// not the user's `pack` declarations.
  system_pack_roots: HashSet<Symbol>,
  /// `true` when the user file is paired with a sibling
  /// `lib.zo` package manifest. Drives the "missing pack
  /// declaration" vs "unresolved module" diagnostic
  /// disambiguation.
  has_lib_zo: bool,
  /// Packs declared `pack foo;` (no `pub`) in lib.zo.
  /// Used to emit `PrivatePackInLoad` rather than
  /// `ModuleNotDeclared` for accidental loads.
  private_packs: HashSet<Symbol>,
  /// Packs declared `pub pack foo;` where `foo/` is a
  /// directory. Holds the SYMBOLS only — the folder's
  /// `.zo` children are resolved through the regular
  /// filesystem path.
  folder_packs: HashSet<Symbol>,
  /// `pub pack foo;` pre-parsed at lib.zo discovery time;
  /// consumed lazily by the lib.zo pack-compile branch
  /// when a `load foo::*;` actually surfaces.
  pending_packs: HashMap<Symbol, PendingPack>,
}

impl DfsCtx {
  fn new() -> Self {
    Self {
      imports: ImportedSymbols::default(),
      module_table_per_path: HashMap::default(),
      folder_aggregations: HashMap::default(),
      parse_cache: HashMap::default(),
      module_sir_instructions: Vec::new(),
      module_next_value_id: 0,
      module_next_label_id: 0,
      system_pack_roots: HashSet::default(),
      has_lib_zo: false,
      private_packs: HashSet::default(),
      folder_packs: HashSet::default(),
      pending_packs: HashMap::default(),
    }
  }
}

impl Compiler {
  /// Creates a new [`Compiler`] instance with the auto-
  /// detected std lib search path. Every caller (zo CLI,
  /// fret build pipeline, integration tests) gets
  /// `preload`/`io`/etc. resolved without per-call wiring.
  pub fn new() -> Self {
    Self {
      stats: Stats::new(),
      profiler: Profiler::new(),
      reporter: Reporter::new(),
      module_resolver: ModuleResolver::new(default_core_search_paths()),
      compiling: HashSet::default(),
      module_table: HashMap::default(),
      emit_json: false,
      snippet_context: 2,
    }
  }

  /// Creates a new [`Compiler`] with explicit search paths for
  /// module resolution.
  pub fn with_search_paths(search_paths: Vec<PathBuf>) -> Self {
    Self {
      stats: Stats::new(),
      profiler: Profiler::new(),
      reporter: Reporter::new(),
      module_resolver: ModuleResolver::new(search_paths),
      compiling: HashSet::default(),
      module_table: HashMap::default(),
      emit_json: false,
      snippet_context: 2,
    }
  }

  /// Applies a [`DiagnosticsConfig`] to this compiler. Bound
  /// from the driver's `--format=json` / `--snippet-context`
  /// / `--explain-decisions` flags in lockstep. State lands
  /// in two places — local fields for `json` / `snippet_context`
  /// and a process-wide atomic for `explain_decisions` (the
  /// rationale channel needs cheap reads from passes in many
  /// crates). The asymmetry is internal; callers see one
  /// uniform setter.
  pub fn configure_diagnostics(&mut self, cfg: DiagnosticsConfig) {
    self.emit_json = cfg.json;
    self.snippet_context = cfg.snippet_context;
    rationale::enable_rationale(cfg.explain_decisions);
  }

  /// Scans a parse tree for `Token::Load` introducer nodes
  /// and extracts module paths from their Ident children.
  fn scan_loads(
    tree: &Tree,
    interner: &mut zo_interner::Interner,
  ) -> Vec<LoadRef> {
    let mut loads = Vec::new();

    for (i, node) in tree.nodes_with_token(Token::Load) {
      if node.child_count == 0 {
        continue;
      }

      let span = tree.spans[i];
      let mut path = Vec::new();
      // Idents appearing inside `(...)` are SELECTIVE
      // items (`load core::regex::(Match, Regex);`),
      // not path segments — they live on the module
      // surface, not on the filesystem. Mirror
      // `execute_load`'s in_selective state so the
      // pre-scan path matches what the resolver
      // expects (`[core, regex]`, not
      // `[core, regex, Match, Regex]`). Without this
      // the resolver chases `core/regex/Match/Regex.zo`
      // and the load fails as `UnresolvedModule`.
      let mut in_selective = false;

      // Pack files share names with primitives (`int.zo`,
      // `str.zo`, `bool.zo`, `char.zo`) — the tokenizer
      // emits `Token::IntType`/`StrType`/… for those, so
      // we re-intern the keyword string into a Symbol.
      for child_idx in node.children_range() {
        let Some(child) = tree.nodes.get(child_idx) else {
          continue;
        };

        match child.token {
          Token::LParen => {
            in_selective = true;
          }
          Token::RParen => {
            // Selective list ends — anything after the
            // `)` is a stray token (today's grammar
            // terminates the load there), so we stop
            // collecting.
            break;
          }
          Token::Star => {
            // Glob terminator — path is complete; no
            // selective items follow.
            break;
          }
          Token::Ident if !in_selective => {
            if let Some(NodeValue::Symbol(sym)) = tree.value(child_idx as u32) {
              path.push(sym);
            }
          }
          _ if !in_selective && child.token.ty_keyword_str().is_some() => {
            let kw = child.token.ty_keyword_str().unwrap();
            path.push(interner.intern(kw));
          }
          _ => {}
        }
      }

      if !path.is_empty() {
        loads.push(LoadRef::new(path, span));
      }
    }

    loads
  }

  /// Checks if `lib.zo` exists alongside the given file.
  fn discover_lib(file_path: &Path) -> Option<PathBuf> {
    let lib_path = file_path.with_file_name("lib.zo");

    if lib_path.is_file() && lib_path != file_path {
      Some(lib_path)
    } else {
      None
    }
  }

  /// Scans a parse tree for `Token::Pack` introducer nodes
  /// and extracts pack names with their visibility.
  fn scan_packs(
    tree: &Tree,
    interner: &zo_interner::Interner,
  ) -> Vec<(String, bool)> {
    let mut packs = Vec::new();

    for (i, _) in tree.nodes_with_token(Token::Pack) {
      if let Some(sym) = tree.first_ident_child_symbol(i) {
        packs.push((interner.get(sym).to_string(), tree.is_pub_at(i)));
      }
    }

    packs
  }

  /// Analyzes a single source file with full module resolution.
  /// Returns the semantic result and tokenization for further
  /// processing (codegen or runtime execution).
  pub fn analyze_source(
    &mut self,
    source: &str,
    file_path: &Path,
  ) -> (SemanticResult, TokenizationResult, ParsingResult, Session) {
    self.profiler.start_phase(TOKENIZER_NAME);
    let mut session = Session::new();
    let tokenizer = Tokenizer::new(source, &mut session.interner);
    let mut tokenization = tokenizer.tokenize();
    self.profiler.end_phase(TOKENIZER_NAME);

    self.profiler.start_phase(PARSER_NAME);
    let parser = Parser::new(&tokenization, source);
    let mut parsing = parser.parse();
    self.profiler.end_phase(PARSER_NAME);

    let mut ctx = DfsCtx::new();

    // Discover lib.zo and parse each pub-pack ONCE here.
    // Compilation is deferred until each pack is actually
    // `load`-ed in the main loop, by which point `ctx.imports`
    // carries preload PLUS the cascaded core packs — so a
    // pack body referencing `showln` / `Vec` / etc. resolves
    // cleanly. The tokenization + parsing + load-scan results
    // are cached on `PendingPack` so the lazy site never
    // re-reads or re-parses the same source file.
    //
    // `ctx.private_packs` collects packs declared as bare
    // `pack foo;` (no `pub`) — skipped by the compile loop
    // but remembered so a later `load foo::*;` from outside
    // can distinguish "private pack" from "undeclared pack"
    // and emit the precise diagnostic. `ctx.folder_packs`
    // collects `pub pack foo;` where `foo/` is a directory;
    // the folder itself has no body to compile and submodules
    // are resolved lazily through the regular filesystem
    // path.
    ctx.has_lib_zo = if let Some(lib_path) = Self::discover_lib(file_path) {
      let lib_source = fs::read_to_string(&lib_path).unwrap_or_default();
      let lib_tokenization =
        Tokenizer::new(&lib_source, &mut session.interner).tokenize();
      let lib_parsing = Parser::new(&lib_tokenization, &lib_source).parse();
      let packs = Self::scan_packs(&lib_parsing.tree, &session.interner);

      let dir = lib_path.parent().unwrap_or(Path::new("."));

      for (pack_name, is_pub) in &packs {
        let pack_file = dir.join(format!("{pack_name}.zo"));
        let pack_folder = dir.join(pack_name);
        let sym = session.interner.intern(pack_name);

        if !*is_pub {
          ctx.private_packs.insert(sym);
        }

        if pack_file.is_file() {
          if !*is_pub {
            continue;
          }

          let pack_source = fs::read_to_string(&pack_file).unwrap_or_default();
          let pack_tok =
            Tokenizer::new(&pack_source, &mut session.interner).tokenize();
          let pack_par = Parser::new(&pack_tok, &pack_source).parse();
          let pack_loads =
            Self::scan_loads(&pack_par.tree, &mut session.interner);

          ctx.pending_packs.insert(
            sym,
            PendingPack {
              path: pack_file,
              tokenization: pack_tok,
              parsing: pack_par,
              loads: pack_loads,
            },
          );
        } else if pack_folder.is_dir() {
          if *is_pub {
            ctx.folder_packs.insert(sym);
          }
        } else {
          report_error(Error::new(
            ErrorKind::PackFileNotFound,
            Span { start: 0, len: 0 },
          ));
        }
      }

      true
    } else {
      false
    };

    // Resolve and compile loaded modules BEFORE analysis.
    // Transitive: we grow `module_paths` as we discover
    // additional loads in each compiled module, so a chain
    // like `main -> misato -> raylib` is fully covered. The
    // `compiling` set both deduplicates and guards against
    // circular ctx.imports.
    let mut module_paths =
      Self::scan_loads(&parsing.tree, &mut session.interner);

    // Frozen snapshot of the user file's top-level `load`s.
    // The user's analyzer seed is built ONLY from these (plus
    // preload). Modules loaded transitively as deps of other
    // modules — but not loaded by the user directly — must
    // never leak into the user's scope.
    let user_top_loads: Vec<LoadRef> = module_paths.clone();

    // System-pack roots bypass the user lib.zo membership
    // check: loads through `core` / `provider` are governed
    // by the std layout, not the user's pack declarations.
    ctx.system_pack_roots = zo_host_paths::SYSTEM_PACK_ROOTS
      .iter()
      .map(|n| session.interner.intern(n))
      .collect();

    // Preload's `exported` scope. Built in two halves:
    // `preload_own` is captured during preload's own
    // compilation (own pubs only — re-export targets aren't
    // compiled yet at that point), and folded with
    // `preload_re_exports` AFTER the main loop populates
    // `ctx.module_table_per_path` with every cascaded
    // module. The combined result becomes zo's prelude.
    // Kept as locals (not in `DfsCtx`) because they're only
    // touched by the preload-pass setup + the post-loop
    // user-seed assembly — they don't ride the recursive
    // module-compile path Phase B / C will lift out.
    let mut preload_own = ImportedSymbols::default();
    let mut preload_re_exports: Vec<Vec<Symbol>> = Vec::new();

    // Auto-import `preload.zo` and its transitive `load`s.
    // preload.zo IS the source of truth for the "always
    // imported" surface — its top-level `load core::…::*;`
    // lines define what's in scope everywhere (zo's
    // equivalent of Rust's prelude). Adding a new
    // "basic" doesn't touch the compiler — just append a
    // `load` line to `preload.zo`. Domain packs (raylib,
    // misato, sqlite, math, …) stay opt-in.
    let preload = ["preload"];

    for module_name in preload {
      let sym = session.interner.intern(module_name);
      let preload_path = vec![sym];

      let resolved = self
        .module_resolver
        .resolve(&preload_path, &session.interner);

      if let Some(m) = resolved {
        let src = m.source.clone();
        let resolved_path = m.path.clone();
        let mut mod_tok =
          Tokenizer::new(&src, &mut session.interner).tokenize();
        let mut mod_par = Parser::new(&mod_tok, &src).parse();

        // Cascade: every `load X::Y::*;` in preload.zo
        // becomes an implicit user load. Prepend so they
        // compile BEFORE any explicit user `load` and their
        // exports are visible to the user file's analyzer
        // (`showln`, `Vec`, `HashMap`, …).
        let mut preload_loads =
          Self::scan_loads(&mod_par.tree, &mut session.interner);

        preload_loads.append(&mut module_paths);
        module_paths = preload_loads;

        let implicit_sym = implicit_pack_for(
          &resolved_path,
          self.module_resolver.search_paths(),
        )
        .map(|stem| session.interner.intern(&stem));

        // Seed each preload pack's analyzer with symbols
        // from earlier preload packs so later packs can
        // use them (e.g. `str.zo` referencing `Option`
        // from `preload.zo` or calling char methods
        // defined in `char.zo`). Clones are small and
        // one-time at startup; without them, each preload
        // runs in isolation and cross-pack references
        // silently emit broken SIR.
        let mut mod_imports = ctx.imports.clone();

        // Splice cumulative generic bodies into THIS
        // module's tree/literals before the analyzer runs,
        // then move the post-splice metadata into
        // `mod_imports.generic_bodies` so `with_imports`
        // can register the lookup tables.
        mod_imports.generic_bodies = splice_generic_bodies(
          &mut mod_par.tree,
          &mut mod_tok.literals,
          std::mem::take(&mut mod_imports.exported_generic_bodies),
        );

        let mod_sem = Analyzer::new(
          &mod_par.tree,
          &mut session.interner,
          &mod_tok.literals,
          &mut session.ty_checker,
        )
        .with_config(AnalyzerConfig {
          imports: mod_imports,
          implicit_pack: implicit_sym,
          source_path: Some(resolved_path.clone()),
          ..AnalyzerConfig::default()
        })
        .analyze();

        let mut exports = extract_exports(
          mod_sem.sir,
          None,
          &session.interner,
          &mod_sem.funs,
          mod_sem.generic_bodies,
          mod_sem.abstract_impls,
        );

        // Each pack's SIR starts its own value-id counter
        // AND label counter at 0. On naive concatenation
        // (many preloaded packs merged before main), pack
        // B's `%0` collides with pack A's `%0`, and the
        // same applies to `Label { id: 0 }` → any
        // `Jump { target: 0 }` / `BranchIfNot { target: 0 }`
        // inside pack B lands on pack A's label. Both
        // produce silent wrong-code. Shift each pack by
        // the running base so the merged module block has
        // unique ids across both namespaces before the
        // final lift above main.
        Sir::offset_value_ids(
          &mut exports.sir_instructions,
          ctx.module_next_value_id,
        );

        Sir::offset_labels(
          &mut exports.sir_instructions,
          ctx.module_next_label_id,
        );

        let mut own_imports = module_exports_to_imports(&exports);
        own_imports.abstract_defs = mod_sem.abstract_defs.clone();

        fold_imports_into(&mut ctx.imports, &own_imports);

        // Stash for the post-loop fold — re-export targets
        // (core::io, core::vec, …) compile AFTER preload, so
        // `ctx.module_table_per_path` is empty here. We resolve
        // `preload_exported` once the table is populated.
        preload_own = own_imports;
        preload_re_exports = exports.re_exports.clone();

        ctx.module_sir_instructions.extend(exports.sir_instructions);

        ctx.module_next_value_id += exports.next_value_id;
        ctx.module_next_label_id += exports.next_label_id;
      }
    }

    // Recursive DFS entry — each top-level load expands
    // its dependency graph post-order via the call stack.
    // `module_paths` here is preload's cascade + the user
    // file's top-level loads, in that order; the
    // recursion's `module_table_per_path` short-circuit
    // keeps re-export hits cheap.
    let module_paths_snapshot = module_paths.clone();
    for module_ref in &module_paths_snapshot {
      self.compile_module_recursive(
        &mut ctx,
        &mut session,
        module_ref.value.clone(),
        module_ref.span,
      );
    }

    // Aggregate folder-namespace exports: every `load X::*;`
    // that expanded into `X/*.zo` children now folds those
    // children's exported scopes back under `X` so the user
    // analyzer's seed can re-export them as one unit.
    for (folder_path, child_paths) in &ctx.folder_aggregations {
      let mut combined = ImportedSymbols::default();

      for child_path in child_paths {
        if let Some(child_exports) = ctx.module_table_per_path.get(child_path) {
          fold_imports_into(&mut combined, child_exports);
        }
      }

      ctx
        .module_table_per_path
        .insert(folder_path.clone(), combined);
    }

    // Analyze with imported symbols pre-loaded.
    self.profiler.start_phase(ANALYZER_NAME);

    // The leaf of each transitively-loaded path IS the pack
    // name (`core::io::*` → `io`). Surfaced so the user
    // analyzer's qualified-call resolution finds them.
    let in_scope_packs: Vec<Symbol> = module_paths
      .iter()
      .filter_map(|m| m.value.last().copied())
      .collect();

    // Resolve preload's exported scope NOW that every
    // re-export target has compiled and populated
    // `ctx.module_table_per_path`. preload is compiled first,
    // before its cascaded targets, so this fold can only be
    // done after the main loop completes.
    let preload_exported = compute_exported_scope(
      preload_own,
      &preload_re_exports,
      &ctx.module_table_per_path,
    );

    // Per-module seed for the user analyzer. ONLY the user's
    // own top-level `load`s contribute, on top of preload's
    // `exported` scope (zo's prelude). A module loaded
    // transitively as another module's private dep never
    // lands here — that's the whole privacy guarantee.
    // `ctx.imports` (the legacy cumulative bag) still seeds every
    // intermediate loaded-module analyzer above; that path
    // stays untouched for now.
    let user_seed = build_module_seed(
      &preload_exported,
      &user_top_loads,
      &ctx.module_table_per_path,
    );

    // Entry programs adopt their file stem as pack identity
    // so top-level `#link { ... }` resolves to a `PackLink`.
    let implicit_sym = file_path
      .file_stem()
      .and_then(|s| s.to_str())
      .map(|stem| session.interner.intern(stem));

    let mut user_seed = user_seed;

    user_seed.generic_bodies = splice_generic_bodies(
      &mut parsing.tree,
      &mut tokenization.literals,
      std::mem::take(&mut user_seed.exported_generic_bodies),
    );

    let analyzer = Analyzer::new(
      &parsing.tree,
      &mut session.interner,
      &tokenization.literals,
      &mut session.ty_checker,
    );

    // The user file's `<img src="…">` and similar
    // path-typed attributes are resolved against this
    // directory at attribute-build time — so the
    // compiled binary holds absolute paths and renders
    // assets regardless of CWD at run time.
    let mut semantic = analyzer
      .with_config(AnalyzerConfig {
        imports: user_seed,
        source_dir: file_path.parent().map(Path::to_path_buf),
        source_path: Some(file_path.to_path_buf()),
        implicit_pack: implicit_sym,
        in_scope_packs,
        is_entry: true,
      })
      .analyze();
    self.profiler.end_phase(ANALYZER_NAME);

    // Merge module SIR into main SIR. Modules must appear
    // before main so their FunDefs are registered before
    // main calls them. All ValueIds are explicit and
    // offsettable — no implicit/explicit mismatch.
    if !ctx.module_sir_instructions.is_empty() {
      let main_next_vid = semantic.sir.next_value_id;
      let main_next_lid = semantic.sir.next_label_id;

      Sir::offset_value_ids(&mut ctx.module_sir_instructions, main_next_vid);
      // Shift module labels above main's own label range
      // (main uses `[0, main_next_lid)`). Per-pack offset
      // above already ensures module labels don't collide
      // with one another; this just lifts the whole module
      // block above main's labels for the merged stream.
      Sir::offset_labels(&mut ctx.module_sir_instructions, main_next_lid);

      // Prepend: modules first, then main.
      let main_insns = std::mem::replace(
        &mut semantic.sir.instructions,
        ctx.module_sir_instructions,
      );

      semantic.sir.instructions.extend(main_insns);
      semantic.sir.next_value_id += ctx.module_next_value_id;
      semantic.sir.next_label_id += ctx.module_next_label_id;
    }

    // Dead code elimination — find main by name.
    let main_sym = session.interner.intern("main");

    // Pin apply-method bodies as DCE roots. The static
    // call graph misses them — `Insn::DynDispatch`
    // resolves through a vtable, not a named Call.
    let dyn_methods_cap: usize = semantic
      .abstract_impls
      .values()
      .map(|im| im.methods.len())
      .sum();
    let mut dyn_methods: Vec<Symbol> = Vec::with_capacity(dyn_methods_cap);
    for im in semantic.abstract_impls.values() {
      dyn_methods.extend(im.methods.iter().copied());
    }

    Dce::new(&mut semantic.sir, main_sym, &session.interner)
      .with_dyn_roots(&dyn_methods)
      .eliminate();

    // Single drain after every analyze-time pass (analyzer,
    // module loads, DCE). One TLS access, not one per pass.
    let tl_errors = zo_reporter::collect_errors();
    if !tl_errors.is_empty() {
      self.reporter.collect_errors(&tl_errors);
    }

    (semantic, tokenization, parsing, session)
  }

  /// Recursive DFS post-order driver. Compiles every
  /// module reachable from `module_path` — including the
  /// lib.zo pack-lookup branch, folder-namespace
  /// expansion, transitive nested loads, and the final
  /// splice+analyze+extract+fold via `compile_module_body`.
  ///
  /// Phase C of the recursive-DFS refactor (PLAN_MODULE_SYSTEM):
  /// replaces the flat `mp_i + module_paths.insert/retain`
  /// hoist gymnastics with the natural call-stack
  /// post-order. A module's nested `load`s recurse first,
  /// so by the time we splice this module's body its
  /// transitive deps already sit in
  /// `ctx.module_table_per_path` — `compile_module_body`'s
  /// `compute_exported_scope` finds every re-export
  /// target without rewind-and-retry.
  ///
  /// Short-circuits if the path is already in the
  /// table — re-export chains that hit the same module
  /// through multiple paths cost a `HashMap::contains_key`
  /// each, not a re-analyze.
  fn compile_module_recursive(
    &mut self,
    ctx: &mut DfsCtx,
    session: &mut Session,
    module_path: Vec<Symbol>,
    load_span: Span,
  ) {
    // Already compiled along an earlier branch — re-export
    // graphs frequently reach the same module twice; the
    // recursive structure deduplicates here so each module
    // analyses exactly once.
    if ctx.module_table_per_path.contains_key(&module_path) {
      return;
    }

    let first_seg = module_path[0];

    // Lazily compile a lib.zo pub-pack the first time it's
    // `load`-ed. Each pack was tokenized + parsed once at
    // lib.zo discovery and stashed in `ctx.pending_packs`;
    // the analysis is the only thing deferred to here so
    // the pack's analyzer sees preload + the cascaded core
    // modules that compiled earlier in this same loop.
    //
    // A pack may `pub load` other lib.zo packs (`a`
    // re-exports `c`). Those targets must compile BEFORE
    // this pack's analyzer runs. The pre-scanned `loads`
    // on `PendingPack` drives the order — no extra tree
    // walks, no re-reads, no re-parses.
    if !self.module_table.contains_key(&first_seg)
      && ctx.pending_packs.contains_key(&first_seg)
    {
      let mut compile_stack: Vec<Symbol> = vec![first_seg];
      let mut visiting: HashSet<Symbol> = HashSet::default();

      while let Some(&top) = compile_stack.last() {
        if self.module_table.contains_key(&top) {
          compile_stack.pop();
          continue;
        }

        let Some(pack) = ctx.pending_packs.get(&top) else {
          compile_stack.pop();
          continue;
        };

        let mut deferred = false;
        for load_ref in &pack.loads {
          let leaf = load_ref.value[0];
          if leaf == top {
            continue;
          }
          if !self.module_table.contains_key(&leaf)
            && ctx.pending_packs.contains_key(&leaf)
            && !visiting.contains(&leaf)
          {
            compile_stack.push(leaf);
            visiting.insert(leaf);
            deferred = true;
          }
        }

        if deferred {
          continue;
        }

        // Move the cached parse out of the map — the pack
        // compiles exactly once, so it's never read again.
        let mut pack = ctx.pending_packs.remove(&top).expect("just verified");

        let implicit_sym =
          implicit_pack_for(&pack.path, self.module_resolver.search_paths())
            .map(|stem| session.interner.intern(&stem));

        let mut pack_imports = ctx.imports.clone();

        pack_imports.generic_bodies = splice_generic_bodies(
          &mut pack.parsing.tree,
          &mut pack.tokenization.literals,
          std::mem::take(&mut pack_imports.exported_generic_bodies),
        );

        let pack_sem = Analyzer::new(
          &pack.parsing.tree,
          &mut session.interner,
          &pack.tokenization.literals,
          &mut session.ty_checker,
        )
        .with_config(AnalyzerConfig {
          imports: pack_imports,
          implicit_pack: implicit_sym,
          source_path: Some(pack.path.clone()),
          ..AnalyzerConfig::default()
        })
        .analyze();

        let mut pack_exports = extract_exports(
          pack_sem.sir,
          None,
          &session.interner,
          &pack_sem.funs,
          pack_sem.generic_bodies,
          pack_sem.abstract_impls,
        );

        // Merge this pack's SIR into the main stream NOW —
        // a pack the user reaches only transitively (e.g.
        // `c` via `a`'s `pub load c::*;`) never gets
        // re-visited from the top-level recursion entry,
        // so without this its `FunDef` bodies vanish from
        // codegen even though their symbols are in scope
        // at the user analyzer. After consuming, we zero
        // the SIR / counter fields so a later double-
        // visit (when the user ALSO directly loads the
        // pack) is a no-op rather than a duplicate emit.
        let mut sir = std::mem::take(&mut pack_exports.sir_instructions);
        Sir::offset_value_ids(&mut sir, ctx.module_next_value_id);
        Sir::offset_labels(&mut sir, ctx.module_next_label_id);
        ctx.module_sir_instructions.extend(sir);
        ctx.module_next_value_id += pack_exports.next_value_id;
        ctx.module_next_label_id += pack_exports.next_label_id;
        pack_exports.next_value_id = 0;
        pack_exports.next_label_id = 0;

        let pack_own = module_exports_to_imports(&pack_exports);
        fold_imports_into(&mut ctx.imports, &pack_own);

        let exported = compute_exported_scope(
          pack_own,
          &pack_exports.re_exports,
          &ctx.module_table_per_path,
        );
        ctx.module_table_per_path.insert(vec![top], exported);

        self.module_table.insert(top, pack_exports);
        visiting.remove(&top);
        compile_stack.pop();
      }
    }

    // Check module_table (populated from lib.zo packs,
    // either eagerly above or lazily on first load).
    if let Some(mut exports) = self.module_table.remove(&first_seg) {
      Sir::offset_labels(
        &mut exports.sir_instructions,
        ctx.module_next_label_id,
      );

      let own_imports = module_exports_to_imports(&exports);

      fold_imports_into(&mut ctx.imports, &own_imports);

      let exported = compute_exported_scope(
        own_imports,
        &exports.re_exports,
        &ctx.module_table_per_path,
      );

      ctx
        .module_table_per_path
        .insert(module_path.clone(), exported);

      ctx.module_sir_instructions.extend(exports.sir_instructions);
      ctx.module_next_value_id += exports.next_value_id;
      ctx.module_next_label_id += exports.next_label_id;

      return;
    }

    // lib.zo exists but module not declared — error.
    // System roots + folder-namespace packs bypass the
    // check (system loads + submodules through the
    // regular file path are always permitted).
    if ctx.has_lib_zo
      && !ctx.system_pack_roots.contains(&first_seg)
      && !ctx.folder_packs.contains(&first_seg)
    {
      let kind = if ctx.private_packs.contains(&first_seg) {
        ErrorKind::PrivatePackInLoad
      } else {
        ErrorKind::ModuleNotDeclared
      };
      report_error(Error::new(kind, load_span));

      return;
    }

    // Fall back to filesystem resolve.
    let (mod_source, selective, resolved_path) = {
      let resolved = self
        .module_resolver
        .resolve(&module_path, &session.interner);

      match resolved {
        Some(m) => {
          (m.source.clone(), m.selective_symbol.clone(), m.path.clone())
        }
        None => {
          // Folder namespace: a `load X::*;` whose tail
          // is a directory expands into one file-load
          // per `.zo` child. Each child compiles via the
          // same recursive entry; the parent's exported
          // scope is then aggregated from the children
          // post-loop via `ctx.folder_aggregations`.
          if let Some(entries) = self
            .module_resolver
            .resolve_folder_entries(&module_path, &session.interner)
          {
            let mut child_paths: Vec<Vec<Symbol>> = Vec::new();

            for entry in &entries {
              let Some(stem) = entry.file_stem().and_then(|s| s.to_str())
              else {
                continue;
              };
              let stem_sym = session.interner.intern(stem);
              let mut child_path = module_path.clone();
              child_path.push(stem_sym);
              child_paths.push(child_path.clone());

              self
                .compile_module_recursive(ctx, session, child_path, load_span);
            }

            ctx
              .folder_aggregations
              .insert(module_path.clone(), child_paths);
            return;
          }

          report_error(Error::new(ErrorKind::UnresolvedModule, load_span));

          return;
        }
      }
    };

    if self.compiling.contains(&resolved_path) {
      report_error(Error::new(ErrorKind::CircularImport, load_span));
      return;
    }

    self.compiling.insert(resolved_path.clone());

    if !ctx.parse_cache.contains_key(&resolved_path) {
      let tok = Tokenizer::new(&mod_source, &mut session.interner).tokenize();
      let par = Parser::new(&tok, &mod_source).parse();
      let tree_baseline = par.tree.baseline();
      let lit_baseline = tok.literals.baseline();

      ctx.parse_cache.insert(
        resolved_path.clone(),
        (tok, par, tree_baseline, lit_baseline),
      );
    }

    // Rewind any splice from a previous analyze pass on
    // this cached tree. The pristine baseline is what the
    // recursive deps below need to scan for nested loads,
    // and what the body-compile method needs as the
    // splice canvas.
    {
      let (mod_tok, mod_par, tree_baseline, lit_baseline) = ctx
        .parse_cache
        .get_mut(&resolved_path)
        .expect("just inserted");

      mod_par.tree.truncate_to(*tree_baseline);
      mod_tok.literals.truncate_to(*lit_baseline);
    }

    // Scan nested loads from the pristine tree, then
    // recursively compile each. The call stack handles
    // the topological order — no manual hoist needed.
    // Self-loops are filtered explicitly so a module
    // that mentions itself in a load doesn't deadlock
    // on `self.compiling`.
    let nested = {
      let (_, mod_par, _, _) =
        ctx.parse_cache.get(&resolved_path).expect("populated");
      Self::scan_loads(&mod_par.tree, &mut session.interner)
    };

    for nested_ref in nested {
      if nested_ref.value == module_path {
        continue;
      }
      self.compile_module_recursive(
        ctx,
        session,
        nested_ref.value,
        nested_ref.span,
      );
    }

    self.compile_module_body(
      ctx,
      session,
      module_path.as_slice(),
      load_span,
      &resolved_path,
      selective.as_deref(),
    );
  }

  /// Splice + analyze + extract + fold for a single
  /// filesystem-resolved module whose deps are already
  /// compiled and cached in `ctx.parse_cache` (the
  /// tree+literals are guaranteed pristine — the driver
  /// `truncate_to(baseline)`s before calling this).
  ///
  /// Phase B of the recursive-DFS refactor: lifts the
  /// per-iteration body out of the flat `analyze_source`
  /// loop so Phase C can call it from a recursive entry
  /// point. The driver still owns:
  ///
  ///   - path resolution + folder-namespace expansion;
  ///   - circular-import detection;
  ///   - parse-cache populate + tree/literal baseline
  ///     rewind;
  ///   - unmet-deps hoist (mp_i / module_paths mutation).
  ///
  /// This method owns ONLY the work that runs after the
  /// dependency graph has been linearised: splicing
  /// generic bodies, running the analyzer, harvesting
  /// exports, raising per-load diagnostics, offsetting
  /// SIR ids, folding into `ctx.imports`, computing the
  /// exported scope, and dropping the `compiling` mark.
  fn compile_module_body(
    &mut self,
    ctx: &mut DfsCtx,
    session: &mut Session,
    module_path: &[Symbol],
    load_span: Span,
    resolved_path: &Path,
    selective: Option<&str>,
  ) {
    let implicit_sym =
      implicit_pack_for(resolved_path, self.module_resolver.search_paths())
        .map(|stem| session.interner.intern(&stem));

    let (mod_tokenization, mod_parsing, _, _) = ctx
      .parse_cache
      .get_mut(resolved_path)
      .expect("driver populated parse_cache before calling");

    // Seed with everything already imported (preload's
    // `Option` / `Result` / `Event` enums + any earlier
    // module's exports). `io.zo`'s `Result<str, int>` and
    // `map.zo`'s `Option<$V>` references resolve against
    // these — without seeding, the loaded module's
    // analyzer reports `Undefined variable` and the user
    // file inherits broken SIR.
    let mut mod_imports = ctx.imports.clone();

    mod_imports.generic_bodies = splice_generic_bodies(
      &mut mod_parsing.tree,
      &mut mod_tokenization.literals,
      std::mem::take(&mut mod_imports.exported_generic_bodies),
    );

    let mod_semantic = Analyzer::new(
      &mod_parsing.tree,
      &mut session.interner,
      &mod_tokenization.literals,
      &mut session.ty_checker,
    )
    .with_config(AnalyzerConfig {
      imports: mod_imports,
      implicit_pack: implicit_sym,
      source_path: Some(resolved_path.to_path_buf()),
      ..AnalyzerConfig::default()
    })
    .analyze();

    let mut exports = extract_exports(
      mod_semantic.sir,
      selective,
      &session.interner,
      &mod_semantic.funs,
      mod_semantic.generic_bodies,
      mod_semantic.abstract_impls,
    );

    // Selective imports that hit a non-pub item — one
    // diagnostic per offending name, anchored at the load
    // span so the user sees which `load M::(foo);` is bad.
    for _hit in &exports.private_selective_hits {
      report_error(Error::new(ErrorKind::PrivateItemInLoad, load_span));
    }

    // A `pub load X::*;` whose target X never landed in
    // the table — X didn't compile (UnresolvedModule /
    // PrivatePackInLoad / etc.), so the re-export chain
    // is broken. Emit at the consumer's load span so the
    // problem surfaces at the link the user touched.
    for re_path in &exports.re_exports {
      if !ctx.module_table_per_path.contains_key(re_path) {
        report_error(Error::new(ErrorKind::ModuleNotReachable, load_span));
      }
    }

    Sir::offset_labels(&mut exports.sir_instructions, ctx.module_next_label_id);

    let mut own_imports = module_exports_to_imports(&exports);
    own_imports.abstract_defs = mod_semantic.abstract_defs.clone();

    fold_imports_into(&mut ctx.imports, &own_imports);

    let exported = compute_exported_scope(
      own_imports,
      &exports.re_exports,
      &ctx.module_table_per_path,
    );

    ctx
      .module_table_per_path
      .insert(module_path.to_vec(), exported);

    ctx.module_sir_instructions.extend(exports.sir_instructions);
    ctx.module_next_value_id += exports.next_value_id;
    ctx.module_next_label_id += exports.next_label_id;

    self.compiling.remove(resolved_path);
  }

  /// Compiles a collections of files based on the [`Target`].
  ///
  /// Output routing mirrors `rustc`:
  ///
  /// * `output_path` is `-o`: an explicit final-binary path.
  ///   When set, it wins for the binary (intermediates still
  ///   route through `out_dir` if set).
  /// * `out_dir` is `--out-dir`: where every other emitted
  ///   file lands (`--emit` dumps and the default binary
  ///   location when `output_path` is `None`). When both are
  ///   `None`, files are written next to each source file —
  ///   same behaviour `rustc foo.rs` has from CWD.
  pub fn compile(
    &mut self,
    files: &[(&PathBuf, String)],
    target: Target,
    stages: &[Stage],
    output_path: &Option<PathBuf>,
    out_dir: Option<&Path>,
  ) -> Result<(), Error> {
    if files.is_empty() {
      return Ok(());
    }

    let should_emit_all = stages.contains(&Stage::All);
    let should_emit_tokens = should_emit_all || stages.contains(&Stage::Tokens);
    let should_emit_tree = should_emit_all || stages.contains(&Stage::Tree);
    let should_emit_sir = should_emit_all || stages.contains(&Stage::Sir);
    let should_emit_asm = should_emit_all || stages.contains(&Stage::Asm);

    self.stats.numlines = files
      .iter()
      .map(|(_, content)| content.lines().count())
      .sum::<usize>();

    self.profiler.set_total_lines(self.stats.numlines);

    // `--out-dir` redirects every emitted file; otherwise
    // each artifact lands next to its source. The directory
    // is created on first use so callers don't need a separate
    // mkdir step before invoking the compiler.
    if let Some(dir) = out_dir
      && let Err(error) = fs::create_dir_all(dir)
    {
      eprintln!("Failed to create out-dir {dir:?}: {error}");
    }

    let resolve_emit_path = |path: &Path, ext: &str| -> PathBuf {
      match out_dir {
        Some(dir) => {
          let stem = path.file_stem().unwrap_or(path.as_os_str());
          dir.join(stem).with_extension(ext)
        }
        None => path.with_extension(ext),
      }
    };

    for (path, code) in files.iter() {
      let (semantic, tokenization, parsing, session) =
        self.analyze_source(code, path);

      self.stats.numtokens += tokenization.tokens.len();
      self.stats.numnodes += parsing.tree.nodes.len();
      self.stats.numinferences += semantic.annotations.len();

      if should_emit_tokens {
        let tokens_path = resolve_emit_path(path, "tokens");
        let mut pp = PrettyPrinter::new();

        pp.format_tokens(&tokenization.tokens, code);

        let tokens_output = pp.finish();

        if let Err(error) = fs::write(&tokens_path, tokens_output) {
          eprintln!("Failed to write tokens to {tokens_path:?}: {error}");
        }
      }

      if should_emit_tree {
        let tree_path = resolve_emit_path(path, "tree");
        let mut pp = PrettyPrinter::new();
        pp.format_tree(&parsing.tree, code);
        let tree_output = pp.finish();

        if let Err(error) = fs::write(&tree_path, tree_output) {
          eprintln!("Failed to write tree to {tree_path:?}: {error}");
        }
      }

      if should_emit_sir {
        let sir_path = resolve_emit_path(path, "sir");
        let mut pp = PrettyPrinter::new();
        pp.format_sir(&semantic.sir, &session.interner);
        let sir_output = pp.finish();

        if let Err(error) = fs::write(&sir_path, sir_output) {
          eprintln!("Failed to write sir to {sir_path:?}: {error}");
        }
      }

      self.profiler.start_phase(CODEGEN_NAME);
      let codegen = Codegen::new(target);

      // ARM64Gen consults this view to drive the generic
      // AAPCS FFI path; CLIF ignores it.
      let type_view =
        Some((session.ty_checker.tys(), &session.ty_checker.ty_table));

      // ARM consumes `abstract_defs` + `abstract_impls`
      // in `emit_vtables`; CLIF ignores them. Build the
      // state lazily — `generate_artifact` (asm dump)
      // and the real `generate` (link object) each get
      // their own snapshot.
      let make_abstract_state = || {
        Some(zo_codegen::AbstractState {
          defs: semantic.abstract_defs.clone(),
          impls: semantic.abstract_impls.clone(),
        })
      };

      if should_emit_asm {
        let artifact = codegen.generate_artifact(
          &session.interner,
          &semantic.sir,
          type_view,
          make_abstract_state(),
        );
        let asm_path = resolve_emit_path(path, "asm");

        let mut pp = PrettyPrinter::new();
        pp.format_asm(&artifact, target);
        let asm_output = pp.finish();

        if let Err(error) = fs::write(&asm_path, asm_output) {
          eprintln!("Failed to write assembly to {asm_path:?}: {error}");
        }
      }

      // Binary destination:
      // 1. explicit `-o <file>` wins, always.
      // 2. else if `--out-dir <dir>` is set, `<dir>/<stem>`.
      // 3. else next to the source, like `rustc foo.rs`.
      let output_path = match (&output_path, out_dir) {
        (Some(p), _) => p.clone(),
        (None, Some(dir)) => {
          let stem = path.file_stem().unwrap_or(path.as_os_str());
          dir.join(stem)
        }
        (None, None) => path.with_extension(""),
      };

      let link_obj = codegen.generate(
        &session.interner,
        &semantic.sir,
        type_view,
        make_abstract_state(),
      );

      self.stats.numartifacts += 1;
      self.profiler.end_phase(CODEGEN_NAME);

      // --- Linker phase ---
      // Pure data transformation: `LinkObject` → executable
      // file. ARM runs the in-process mach-o assembler;
      // CLIF shells out to `cc`. Either way, no codegen
      // state crosses this boundary — every fixup, symbol
      // table, and entry-point offset already lives on
      // `link_obj`.
      self.profiler.start_phase(LINKER_NAME);

      if let Err(err) = zo_linker::link(link_obj, &output_path, target) {
        eprintln!("zo: link failed: {err}");
      } else {
        self.stats.numlinked += 1;
      }

      // Colocate runtime dylibs that the compiled
      // binary references at `@executable_path/`. zo's
      // runtimes are context-dependent — concurrency,
      // native UI, and web are three independent
      // artifacts. A program that only prints strings
      // needs none; a program that `spawn`s tasks
      // needs the concurrency dylib; a templating
      // program needs the UI dylib. Detection is on
      // the SIR emitted by the executor — each
      // concurrency / UI insn maps to a set of
      // runtime-symbol imports, and the codegen
      // already gates the `LC_LOAD_DYLIB` entry on
      // those same imports. We just mirror that gate
      // here to stage the matching dylib file.
      stage_runtime_artifacts(&semantic.sir, &session.interner, &output_path);

      self.profiler.end_phase(LINKER_NAME);
      self.profiler.set_output(path.display().to_string());
    }

    let errors = self.reporter.errors();

    if !errors.is_empty() {
      let mut aggregator = ErrorAggregator::new();

      aggregator.add_errors(errors);

      for (path, source) in files.iter() {
        let filename = path
          .file_name()
          .map(|n| n.to_string_lossy())
          .unwrap_or_else(|| path.to_string_lossy());

        let _ = if self.emit_json {
          json::to_stdout(&aggregator, source, &filename, self.snippet_context)
        } else {
          render_errors_to_stderr(&aggregator, source, &filename)
        };
      }

      aggregator.clear();

      // Warnings are surfaced above but do not fail the build.
      // Only hard errors (`Severity::Error`) propagate as a
      // compilation failure.
      let has_hard_error = errors
        .iter()
        .any(|e| matches!(e.severity(), Severity::Error));

      if has_hard_error {
        return Err(Error::new(ErrorKind::InternalCompilerError, Span::ZERO));
      }
    }

    self.profiler.set_tokens_count(self.stats.numtokens);
    self.profiler.set_nodes_count(self.stats.numnodes);
    self.profiler.set_inferences_count(self.stats.numinferences);
    self.profiler.set_artifacts_count(self.stats.numartifacts);
    self.profiler.set_artifacts_linked(self.stats.numlinked);
    // Profiler text writes to stdout — when `--format=json`
    // is active, that bleed corrupts the NDJSON stream a
    // consumer is parsing. Suppress; the timing data is
    // human-only.
    if !self.emit_json {
      self.profiler.summary(target.name());
    }

    Ok(())
  }
}
impl Default for Compiler {
  fn default() -> Self {
    Self::new()
  }
}

/// Represents the compiler [`Stats`].
pub struct Stats {
  /// The number of lines.
  numlines: usize,
  /// The number of tokens.
  numtokens: usize,
  /// The number of nodes.
  numnodes: usize,
  /// The number of annotations.
  numinferences: usize,
  /// The number of artifacts emitted by the codegen phase.
  numartifacts: usize,
  /// The number of artifacts the linker turned into
  /// runnable executables. Tracked separately from
  /// `numartifacts` so a link failure leaves an
  /// observable gap in the profiler summary.
  numlinked: usize,
}
impl Stats {
  /// Creates a new [`Stats`] instance.
  pub fn new() -> Self {
    Self {
      numlines: 0,
      numtokens: 0,
      numnodes: 0,
      numinferences: 0,
      numartifacts: 0,
      numlinked: 0,
    }
  }
}

/// Runtime context a compiled binary depends on.
/// Orthogonal flags — a program may pull in zero, one,
/// or several runtimes (e.g. a UI program that also
/// spawns background tasks). `dylib_basenames` carries the
/// `@executable_path/...` dylib basenames extracted from
/// `Insn::PackLink` (per-pack `#link { ... }`); each one
/// gets staged next to the user binary so dyld can
/// resolve it.
#[derive(Default, Clone)]
struct RuntimeNeeds {
  concurrency: bool,
  native_ui: bool,
  web_ui: bool,
  dylib_basenames: Vec<String>,
}

/// Call-name prefixes that trigger runtime-dylib staging.
/// Any `Insn::Call` whose mangled name starts with one of
/// these resolves to a `_zo_*` symbol in
/// `libzo_runtime.dylib`.
const RUNTIME_DYLIB_PREFIXES: &[&str] = &[
  "HashMap::",
  "HashSet::",
  "Vec::",
  "__zo_map_",
  "__zo_vec_",
  "__zo_set_",
];

/// Exact call names that trigger runtime-dylib staging.
/// Use the prefix table above when a call family shares
/// a stem; this list is for one-off intrinsics.
const RUNTIME_DYLIB_NAMES: &[&str] = &[
  "arr_int::sort",
  "read",
  "readln",
  "args",
  "__zo_str_replace",
];

impl RuntimeNeeds {
  fn from_sir(sir: &Sir, interner: &zo_interner::Interner) -> Self {
    let mut needs = Self::default();

    for insn in &sir.instructions {
      match insn {
        zo_sir::Insn::ChannelCreate { .. }
        | zo_sir::Insn::ChannelSend { .. }
        | zo_sir::Insn::ChannelRecv { .. }
        | zo_sir::Insn::ChannelClose { .. }
        | zo_sir::Insn::TaskSpawn { .. }
        | zo_sir::Insn::TaskAwait { .. }
        | zo_sir::Insn::TaskCancelled { .. }
        | zo_sir::Insn::TaskCancel { .. }
        | zo_sir::Insn::SelectWait { .. }
        | zo_sir::Insn::NurseryBegin { .. }
        | zo_sir::Insn::NurseryEnd { .. }
        | zo_sir::Insn::StrSlice { .. } => {
          needs.concurrency = true;
        }
        zo_sir::Insn::Call { name, .. } => {
          let n = interner.get(*name);

          if RUNTIME_DYLIB_PREFIXES.iter().any(|p| n.starts_with(p))
            || RUNTIME_DYLIB_NAMES.contains(&n)
          {
            needs.concurrency = true;
          }
        }
        zo_sir::Insn::Template { .. } => {
          // Template programs run in-process through
          // `zo run` today; a future `zo build`
          // web-target would flip `web_ui` here and
          // emit `bridge.js` alongside the binary.
          needs.native_ui = true;
        }
        zo_sir::Insn::PackLink {
          resolution: zo_sir::LinkResolution::Resolved(sym),
          ..
        } => {
          // The executor pre-resolved `system → vendor`;
          // we stage whatever it produced.
          // `@executable_path/<name>` entries need the
          // file copied next to the user binary; absolute
          // system paths (`/opt/...`, `/usr/...`) are
          // resolved by dyld at load time and need no
          // staging.
          if let Some(rest) =
            interner.get(*sym).strip_prefix("@executable_path/")
          {
            needs.dylib_basenames.push(rest.to_owned());
          }
        }
        _ => {}
      }
    }

    needs
  }
}

// Per-host file names — `.dylib` on macOS, `.so` on Linux.
#[cfg(target_os = "macos")]
const CONCURRENCY_DYLIB: &str = "libzo_runtime.dylib";
#[cfg(target_os = "linux")]
const CONCURRENCY_DYLIB: &str = "libzo_runtime.so";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const CONCURRENCY_DYLIB: &str = "libzo_runtime.dylib";

/// Materialise each `@executable_path/<dylib>` reference
/// the linker emitted by copying that dylib next to the
/// produced user binary. Sourced from the sibling
/// directory of the running `zo` compiler (e.g.
/// `target/debug/`). No-op when no runtime is needed or
/// when the source dylib is missing.
fn stage_runtime_artifacts(
  sir: &Sir,
  interner: &zo_interner::Interner,
  output_path: &std::path::Path,
) {
  let needs = RuntimeNeeds::from_sir(sir, interner);

  if !needs.concurrency
    && !needs.native_ui
    && !needs.web_ui
    && needs.dylib_basenames.is_empty()
  {
    return;
  }

  let Some(output_dir) = output_path.parent() else {
    return;
  };

  let Ok(zo_binary) = std::env::current_exe() else {
    return;
  };

  let Some(runtime_dir) = zo_binary.parent() else {
    return;
  };

  // The `deps/` sibling is the invariant location for
  // every staged runtime dylib — pairs with the linker's
  // `@loader_path/deps/<name>` `LC_LOAD_DYLIB`. Created
  // lazily so binaries that need no runtime never get an
  // empty directory.
  let deps_dir = output_dir.join("deps");

  if let Err(error) = std::fs::create_dir_all(&deps_dir) {
    eprintln!("Failed to create deps dir {deps_dir:?}: {error}");
    return;
  }

  if needs.concurrency {
    stage_dylib(runtime_dir, &deps_dir, CONCURRENCY_DYLIB);
  }

  // Native / web UI staging will land here when those
  // runtimes become separate dylibs referenced by the
  // binary (today they run in-process via `zo run`).

  for name in &needs.dylib_basenames {
    stage_dylib(runtime_dir, &deps_dir, name);
  }
}

/// Copy one runtime dylib next to a freshly-built user
/// binary.
///
/// Prefers `deps/<name>` over the sibling
/// `<runtime_dir>/<name>` because cargo only restages the
/// sibling when the cdylib's owning package is built
/// directly; a transitive build through `--bin zo`
/// refreshes `deps/` but leaves the sibling stale.
///
/// Re-copy is unconditional. A `(size, mtime)` skip
/// shortcut once left stale dylibs after a git checkout
/// where two builds landed in the same minute with the
/// same byte count but different `LC_LOAD_DYLIB` layouts —
/// dyld silently hangs in that case.
fn stage_dylib(
  runtime_dir: &std::path::Path,
  output_dir: &std::path::Path,
  name: &str,
) {
  // Search order:
  // 1. `deps/<name>` — cargo build artifacts that the
  //    runtime crate produces (libzo_runtime,
  //    libzo_misato).
  // 2. `<runtime_dir>/<name>` — the sibling copy cargo
  //    stages on direct cdylib builds (stale in
  //    transitive builds, see below).
  // 3. `<runtime_dir>/../lib/vendor/<name>` — F7
  //    vendored prebuilts (raylib, future C libs)
  //    placed by `tasks/zo-install.sh` or staged
  //    manually under `target/lib/vendor/` for local
  //    development.
  let candidates = [
    runtime_dir.join("deps").join(name),
    runtime_dir.join(name),
    runtime_dir.join("..").join("lib").join("vendor").join(name),
  ];

  // Race-safe staging: copy to a PID-stamped tempfile,
  // then `rename` over the destination. The test runner
  // spawns ~400 parallel `zo build` processes into one
  // tmp directory; a plain `fs::copy` (truncate +
  // sequential write) lets two writers interleave and
  // leaves dyld mapping a torn dylib — the loaded test
  // binary then SIGKILLs at launch. POSIX `rename` is
  // atomic on the same filesystem, so concurrent readers
  // see either the old inode or the new inode, never a
  // partial one.
  for src in &candidates {
    if src.exists() {
      let dest = output_dir.join(name);
      let tmp =
        output_dir.join(format!(".{}.{}.tmp", name, std::process::id(),));

      if std::fs::copy(src, &tmp).is_ok() {
        let _ = std::fs::rename(&tmp, &dest);
      } else {
        let _ = std::fs::remove_file(&tmp);
      }

      return;
    }
  }
}
