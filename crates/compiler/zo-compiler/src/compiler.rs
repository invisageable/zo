//! ```sh
//! cargo run --release --bin zon -- build zo-samples/tests/test_1000000_funcs.zo --target arm64-apple-darwin
//! ```

use crate::constants::{
  ANALYZER_NAME, CODEGEN_NAME, LINKER_NAME, PARSER_NAME, TOKENIZER_NAME,
};

use crate::stage::Stage;

use zo_analyzer::{Analyzer, AnalyzerConfig, ImportedSymbols, SemanticResult};
use zo_codegen::codegen::Codegen;
use zo_codegen_backend::Target;
use zo_dce::Dce;
use zo_error::{Error, ErrorKind, Severity};
use zo_interner::Symbol;
use zo_module_resolver::{ModuleExports, ModuleResolver, extract_exports};
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
use zo_token::Token;
use zo_tokenizer::{TokenizationResult, Tokenizer};
use zo_tree::{NodeValue, Tree};
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

/// File-as-pack rule: returns the implicit pack name for a
/// loaded module file, or `None` when the file is a package
/// manifest (`lib.zo`) or a binary entry (`main.zo`).
///
/// Two cases:
///   1. File directly under a search-path root (`core/math.zo`)
///      → use the file stem. Each file is its own pack.
///   2. File inside a sub-folder namespace
///      (`provider/raylib/rcore.zo`) → use the parent folder
///      name. Every `.zo` in that folder shares the pack
///      identity so `#link` placed in any one file covers all
///      sibling `pub ffi` declarations.
fn implicit_pack_for<'a>(
  path: &'a Path,
  search_paths: &[PathBuf],
) -> Option<&'a str> {
  let stem = path.file_stem().and_then(|s| s.to_str())?;

  if matches!(stem, "lib" | "main") {
    return None;
  }

  let parent = path.parent()?;

  if search_paths.iter().any(|sp| sp == parent) {
    return Some(stem);
  }

  parent.file_name().and_then(|n| n.to_str())
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
  into.funs.extend(other.funs.iter().cloned());
  into.vars.extend(other.vars.iter().cloned());
  into.enums.extend(other.enums.iter().cloned());

  for (sym, def) in &other.abstract_defs {
    into.abstract_defs.insert(*sym, def.clone());
  }
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

      // Pack files share names with primitives (`int.zo`,
      // `str.zo`, `bool.zo`, `char.zo`) — the tokenizer
      // emits `Token::IntType`/`StrType`/… for those, so
      // we re-intern the keyword string into a Symbol.
      for child_idx in node.children_range() {
        let Some(child) = tree.nodes.get(child_idx) else {
          continue;
        };

        if child.token == Token::Ident {
          if let Some(NodeValue::Symbol(sym)) = tree.value(child_idx as u32) {
            path.push(sym);
          }
        } else if let Some(kw) = child.token.ty_keyword_str() {
          path.push(interner.intern(kw));
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
    let tokenization = tokenizer.tokenize();
    self.profiler.end_phase(TOKENIZER_NAME);

    self.profiler.start_phase(PARSER_NAME);
    let parser = Parser::new(&tokenization, source);
    let parsing = parser.parse();
    self.profiler.end_phase(PARSER_NAME);

    // Packs declared as bare `pack foo;` (no `pub`) in
    // lib.zo. They're skipped over by the compile loop but
    // remembered here so a later `load foo::*;` from outside
    // can distinguish "private pack" from "undeclared pack"
    // and emit the precise diagnostic.
    let mut private_packs: HashSet<Symbol> = HashSet::default();

    // Discover lib.zo and parse each pub-pack ONCE here.
    // Compilation is deferred until each pack is actually
    // `load`-ed in the main loop, by which point `imports`
    // carries preload PLUS the cascaded core packs — so a
    // pack body referencing `showln` / `Vec` / etc. resolves
    // cleanly. The tokenization + parsing + load-scan results
    // are cached on `PendingPack` so the lazy site never
    // re-reads or re-parses the same source file.
    let mut pending_packs: HashMap<Symbol, PendingPack> = HashMap::default();
    // Folder-namespace packs declared as `pub pack foo;` where
    // `foo/` is a directory containing submodule `.zo` files.
    // The folder itself has no body to compile — submodules
    // are resolved lazily through the regular file path.
    let mut folder_packs: HashSet<Symbol> = HashSet::default();
    let has_lib_zo = if let Some(lib_path) = Self::discover_lib(file_path) {
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
          private_packs.insert(sym);
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

          pending_packs.insert(
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
            folder_packs.insert(sym);
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
    // circular imports.
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
    let system_pack_roots: HashSet<Symbol> = zo_host_paths::SYSTEM_PACK_ROOTS
      .iter()
      .map(|n| session.interner.intern(n))
      .collect();
    // Cumulative imports — every loaded module sees what
    // earlier modules exported (preload's `Option`/`Result`
    // enums, core::int's primitive methods, etc.) and
    // contributes its own exports. Cloned per loaded-module
    // analyzer construction; ownership flows into the final
    // user analyzer at the end.
    let mut imports = ImportedSymbols {
      funs: Vec::new(),
      vars: Vec::new(),
      enums: Vec::new(),
      abstract_defs: HashMap::default(),
    };

    // Per-path `exported` scope table. Populated alongside
    // `imports` as each module compiles; the user analyzer's
    // seed is then built from this so it sees ONLY what its
    // own loads (plus preload) re-export.
    let mut module_table_per_path: HashMap<Vec<Symbol>, ImportedSymbols> =
      HashMap::default();

    // Folder-namespace expansions: when `load foo::*;` hits a
    // directory, each `.zo` child is queued as a separate
    // module load and the folder path is remembered here so
    // the children's exports can be combined back under
    // `["foo"]` before the user analyzer's seed is built.
    let mut folder_aggregations: HashMap<Vec<Symbol>, Vec<Vec<Symbol>>> =
      HashMap::default();

    // Preload's `exported` scope. Built in two halves:
    // `preload_own` is captured during preload's own
    // compilation (own pubs only — re-export targets aren't
    // compiled yet at that point), and folded with
    // `preload_re_exports` AFTER the main loop populates
    // `module_table_per_path` with every cascaded module.
    // The combined result becomes zo's prelude.
    let mut preload_own = ImportedSymbols {
      funs: Vec::new(),
      vars: Vec::new(),
      enums: Vec::new(),
      abstract_defs: HashMap::default(),
    };
    let mut preload_re_exports: Vec<Vec<Symbol>> = Vec::new();

    // Cache parsed modules so the topological-hoist rewind
    // doesn't re-tokenize + re-parse the same file. Cheap
    // on the first visit (Tree owns its data, no source
    // borrows) and saves a full front-end pass per
    // deferred module.
    let mut parse_cache: HashMap<PathBuf, (TokenizationResult, ParsingResult)> =
      HashMap::default();

    let mut module_sir_instructions = Vec::new();
    let mut module_next_value_id: u32 = 0;
    let mut module_next_label_id: u32 = 0;

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
        let mod_tok = Tokenizer::new(&src, &mut session.interner).tokenize();
        let mod_par = Parser::new(&mod_tok, &src).parse();

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
        .map(|stem| session.interner.intern(stem));

        // Seed each preload pack's analyzer with symbols
        // from earlier preload packs so later packs can
        // use them (e.g. `str.zo` referencing `Option`
        // from `preload.zo` or calling char methods
        // defined in `char.zo`). Clones are small and
        // one-time at startup; without them, each preload
        // runs in isolation and cross-pack references
        // silently emit broken SIR.
        let mod_sem = Analyzer::new(
          &mod_par.tree,
          &mut session.interner,
          &mod_tok.literals,
          &mut session.ty_checker,
        )
        .with_config(AnalyzerConfig {
          imports: imports.clone(),
          implicit_pack: implicit_sym,
          ..AnalyzerConfig::default()
        })
        .analyze();

        let mut exports =
          extract_exports(mod_sem.sir, None, &session.interner, &mod_sem.funs);

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
          module_next_value_id,
        );

        Sir::offset_labels(&mut exports.sir_instructions, module_next_label_id);

        let mut own_imports = module_exports_to_imports(&exports);
        own_imports.abstract_defs = mod_sem.abstract_defs.clone();

        fold_imports_into(&mut imports, &own_imports);

        // Stash for the post-loop fold — re-export targets
        // (core::io, core::vec, …) compile AFTER preload, so
        // `module_table_per_path` is empty here. We resolve
        // `preload_exported` once the table is populated.
        preload_own = own_imports;
        preload_re_exports = exports.re_exports.clone();

        module_sir_instructions.extend(exports.sir_instructions);

        module_next_value_id += exports.next_value_id;
        module_next_label_id += exports.next_label_id;
      }
    }

    let mut mp_i = 0;
    while mp_i < module_paths.len() {
      let LoadRef {
        value: module_path,
        span: load_span,
      } = module_paths[mp_i].clone();
      mp_i += 1;

      let first_seg = module_path[0];
      let _mod_name = session.interner.get(first_seg).to_owned();

      // Lazily compile a lib.zo pub-pack the first time it's
      // `load`-ed. Each pack was tokenized + parsed once at
      // lib.zo discovery and stashed in `pending_packs`; the
      // analysis is the only thing deferred to here so the
      // pack's analyzer sees preload + the cascaded core
      // modules that compiled earlier in this same loop.
      //
      // A pack may `pub load` other lib.zo packs (`a`
      // re-exports `c`). Those targets must compile BEFORE
      // this pack's analyzer runs. The pre-scanned `loads`
      // on `PendingPack` drives the order — no extra tree
      // walks, no re-reads, no re-parses.
      if !self.module_table.contains_key(&first_seg)
        && pending_packs.contains_key(&first_seg)
      {
        let mut compile_stack: Vec<Symbol> = vec![first_seg];
        let mut visiting: HashSet<Symbol> = HashSet::default();

        while let Some(&top) = compile_stack.last() {
          if self.module_table.contains_key(&top) {
            compile_stack.pop();
            continue;
          }

          let Some(pack) = pending_packs.get(&top) else {
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
              && pending_packs.contains_key(&leaf)
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
          let pack = pending_packs.remove(&top).expect("just verified");

          let implicit_sym =
            implicit_pack_for(&pack.path, self.module_resolver.search_paths())
              .map(|stem| session.interner.intern(stem));

          let pack_sem = Analyzer::new(
            &pack.parsing.tree,
            &mut session.interner,
            &pack.tokenization.literals,
            &mut session.ty_checker,
          )
          .with_config(AnalyzerConfig {
            imports: imports.clone(),
            implicit_pack: implicit_sym,
            ..AnalyzerConfig::default()
          })
          .analyze();

          let mut pack_exports = extract_exports(
            pack_sem.sir,
            None,
            &session.interner,
            &pack_sem.funs,
          );

          // Merge this pack's SIR into the main stream NOW —
          // a pack the user reaches only transitively (e.g.
          // `c` via `a`'s `pub load c::*;`) never gets
          // visited by the main loop's lib.zo branch, so
          // without this its `FunDef` bodies vanish from
          // codegen even though their symbols are in scope
          // at the user analyzer. After consuming, we zero
          // the SIR / counter fields so a later double-
          // visit (when the user ALSO directly loads the
          // pack) is a no-op rather than a duplicate emit.
          let mut sir = std::mem::take(&mut pack_exports.sir_instructions);
          Sir::offset_value_ids(&mut sir, module_next_value_id);
          Sir::offset_labels(&mut sir, module_next_label_id);
          module_sir_instructions.extend(sir);
          module_next_value_id += pack_exports.next_value_id;
          module_next_label_id += pack_exports.next_label_id;
          pack_exports.next_value_id = 0;
          pack_exports.next_label_id = 0;

          // Fold this pack's exports into the cumulative
          // `imports` so the NEXT pack on the stack (which
          // re-exports it) sees its symbols.
          let pack_own = module_exports_to_imports(&pack_exports);
          fold_imports_into(&mut imports, &pack_own);

          // Build the pack's `exported` scope (own pubs +
          // re-exported targets' scopes) and key it by the
          // single-segment path the user writes for a
          // lib.zo pack (`load <name>`). Required so the
          // user analyzer's seed can fold the pack's
          // re-exports — the main loop's lib.zo branch
          // doesn't compute this for packs the user only
          // reaches transitively.
          let exported = compute_exported_scope(
            pack_own,
            &pack_exports.re_exports,
            &module_table_per_path,
          );
          module_table_per_path.insert(vec![top], exported);

          self.module_table.insert(top, pack_exports);
          visiting.remove(&top);
          compile_stack.pop();
        }
      }

      // Check module_table (populated from lib.zo packs,
      // either eagerly above or lazily on first load).
      if let Some(mut exports) = self.module_table.remove(&first_seg) {
        Sir::offset_labels(&mut exports.sir_instructions, module_next_label_id);

        let own_imports = module_exports_to_imports(&exports);

        fold_imports_into(&mut imports, &own_imports);

        let exported = compute_exported_scope(
          own_imports,
          &exports.re_exports,
          &module_table_per_path,
        );

        module_table_per_path.insert(module_path.clone(), exported);

        module_sir_instructions.extend(exports.sir_instructions);

        module_next_value_id += exports.next_value_id;
        module_next_label_id += exports.next_label_id;

        continue;
      }

      // lib.zo exists but module not declared — error.
      // System roots + folder-namespace packs bypass the
      // check (system loads + submodules through the
      // regular file path are always permitted).
      if has_lib_zo
        && !system_pack_roots.contains(&first_seg)
        && !folder_packs.contains(&first_seg)
      {
        let kind = if private_packs.contains(&first_seg) {
          ErrorKind::PrivatePackInLoad
        } else {
          ErrorKind::ModuleNotDeclared
        };
        report_error(Error::new(kind, load_span));

        continue;
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
            // Folder namespace: a `load X::*;` whose tail is a
            // directory expands into one file-load per `.zo`
            // child. The folder path is recorded so the
            // children's exports can be aggregated back under
            // it before the user analyzer runs.
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
                module_paths.push(LoadRef::new(child_path, load_span));
              }

              folder_aggregations.insert(module_path.clone(), child_paths);
              continue;
            }

            report_error(Error::new(ErrorKind::UnresolvedModule, load_span));

            continue;
          }
        }
      };

      if self.compiling.contains(&resolved_path) {
        report_error(Error::new(ErrorKind::CircularImport, load_span));
        continue;
      }

      self.compiling.insert(resolved_path.clone());

      if !parse_cache.contains_key(&resolved_path) {
        let tok = Tokenizer::new(&mod_source, &mut session.interner).tokenize();
        let par = Parser::new(&tok, &mod_source).parse();
        parse_cache.insert(resolved_path.clone(), (tok, par));
      }

      let (mod_tokenization, mod_parsing) =
        parse_cache.get(&resolved_path).expect("just inserted");

      // Topological hoist: any `load` in this module that
      // hasn't been compiled yet must compile FIRST, or
      // this module's analyzer hits `find_fun(c_str) →
      // None` and falls into the "function not found"
      // path — which emits the Call without pushing a
      // return value, breaking the outer arg pop in
      // nested forms like `my_open(c_str(path))`.
      let nested = Self::scan_loads(&mod_parsing.tree, &mut session.interner);

      let mut unmet: Vec<LoadRef> = Vec::new();

      for nested_ref in nested {
        // Skip self-loops (a module that mentions itself).
        if nested_ref.value == module_path {
          continue;
        }

        if !module_paths[..mp_i]
          .iter()
          .any(|q| q.value == nested_ref.value)
        {
          unmet.push(nested_ref);
        }
      }

      if !unmet.is_empty() {
        // Restore `compiling` (we're not actually compiling
        // this yet — we'll come back).
        self.compiling.remove(&resolved_path);

        // For each unmet dep: if it's already queued at a
        // later position, remove it (we'll re-insert
        // before the current module so it processes first).
        // The bare `Vec::retain` is O(n), but module_paths
        // is small (one entry per loaded pack).
        for dep in &unmet {
          module_paths.retain(|q| q.value != dep.value);
        }

        // Insert each unmet dep right before the current
        // module (which is at `mp_i - 1`), preserving order.
        let insert_at = mp_i - 1;

        for (i, dep) in unmet.into_iter().enumerate() {
          module_paths.insert(insert_at + i, dep);
        }

        // Rewind so we revisit this module after its deps.
        mp_i = insert_at;

        continue;
      }

      let implicit_sym =
        implicit_pack_for(&resolved_path, self.module_resolver.search_paths())
          .map(|stem| session.interner.intern(stem));

      // Seed with everything already imported (preload's
      // `Option`/`Result`/`Event` enums + any earlier
      // module's exports). io.zo's `Result<str, int>` and
      // map.zo's `Option<$V>` references resolve against
      // these — without seeding, the loaded module's
      // analyzer reports `Undefined variable` and the
      // user file inherits broken SIR.
      let mod_semantic = Analyzer::new(
        &mod_parsing.tree,
        &mut session.interner,
        &mod_tokenization.literals,
        &mut session.ty_checker,
      )
      .with_config(AnalyzerConfig {
        imports: imports.clone(),
        implicit_pack: implicit_sym,
        ..AnalyzerConfig::default()
      })
      .analyze();

      let mut exports = extract_exports(
        mod_semantic.sir,
        selective.as_deref(),
        &session.interner,
        &mod_semantic.funs,
      );

      // Selective imports that hit a non-pub item — one
      // diagnostic per offending name, anchored at the load
      // span so the user sees which `load M::(foo);` is bad.
      for _hit in &exports.private_selective_hits {
        report_error(Error::new(ErrorKind::PrivateItemInLoad, load_span));
      }

      // A `pub load X::*;` whose target X never landed in the
      // table — X didn't compile (UnresolvedModule /
      // PrivatePackInLoad / etc.), so the re-export chain is
      // broken. Emit at the consumer's load span so the
      // problem surfaces at the link the user touched.
      for re_path in &exports.re_exports {
        if !module_table_per_path.contains_key(re_path) {
          report_error(Error::new(ErrorKind::ModuleNotReachable, load_span));
        }
      }

      Sir::offset_labels(&mut exports.sir_instructions, module_next_label_id);

      let mut own_imports = module_exports_to_imports(&exports);
      own_imports.abstract_defs = mod_semantic.abstract_defs.clone();

      fold_imports_into(&mut imports, &own_imports);

      let exported = compute_exported_scope(
        own_imports,
        &exports.re_exports,
        &module_table_per_path,
      );

      module_table_per_path.insert(module_path.clone(), exported);

      module_sir_instructions.extend(exports.sir_instructions);
      module_next_value_id += exports.next_value_id;
      module_next_label_id += exports.next_label_id;

      self.compiling.remove(&resolved_path);
    }

    // Aggregate folder-namespace exports: every `load X::*;`
    // that expanded into `X/*.zo` children now folds those
    // children's exported scopes back under `X` so the user
    // analyzer's seed can re-export them as one unit.
    for (folder_path, child_paths) in &folder_aggregations {
      let mut combined = ImportedSymbols::default();

      for child_path in child_paths {
        if let Some(child_exports) = module_table_per_path.get(child_path) {
          fold_imports_into(&mut combined, child_exports);
        }
      }

      module_table_per_path.insert(folder_path.clone(), combined);
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
    // `module_table_per_path`. preload is compiled first,
    // before its cascaded targets, so this fold can only be
    // done after the main loop completes.
    let preload_exported = compute_exported_scope(
      preload_own,
      &preload_re_exports,
      &module_table_per_path,
    );

    // Per-module seed for the user analyzer. ONLY the user's
    // own top-level `load`s contribute, on top of preload's
    // `exported` scope (zo's prelude). A module loaded
    // transitively as another module's private dep never
    // lands here — that's the whole privacy guarantee.
    // `imports` (the legacy cumulative bag) still seeds every
    // intermediate loaded-module analyzer above; that path
    // stays untouched for now.
    let user_seed = build_module_seed(
      &preload_exported,
      &user_top_loads,
      &module_table_per_path,
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
        implicit_pack: None,
        in_scope_packs,
        is_entry: true,
      })
      .analyze();
    self.profiler.end_phase(ANALYZER_NAME);

    // Merge module SIR into main SIR. Modules must appear
    // before main so their FunDefs are registered before
    // main calls them. All ValueIds are explicit and
    // offsettable — no implicit/explicit mismatch.
    if !module_sir_instructions.is_empty() {
      let main_next_vid = semantic.sir.next_value_id;
      let main_next_lid = semantic.sir.next_label_id;

      Sir::offset_value_ids(&mut module_sir_instructions, main_next_vid);
      // Shift module labels above main's own label range
      // (main uses `[0, main_next_lid)`). Per-pack offset
      // above already ensures module labels don't collide
      // with one another; this just lifts the whole module
      // block above main's labels for the merged stream.
      Sir::offset_labels(&mut module_sir_instructions, main_next_lid);

      // Prepend: modules first, then main.
      let main_insns = std::mem::replace(
        &mut semantic.sir.instructions,
        module_sir_instructions,
      );

      semantic.sir.instructions.extend(main_insns);
      semantic.sir.next_value_id += module_next_value_id;
      semantic.sir.next_label_id += module_next_label_id;
    }

    // Dead code elimination — find main by name.
    let main_sym = session.interner.intern("main");

    Dce::new(&mut semantic.sir, main_sym, &session.interner).eliminate();

    // Single drain after every analyze-time pass (analyzer,
    // module loads, DCE). One TLS access, not one per pass.
    let tl_errors = zo_reporter::collect_errors();
    if !tl_errors.is_empty() {
      self.reporter.collect_errors(&tl_errors);
    }

    (semantic, tokenization, parsing, session)
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

      if should_emit_asm {
        let artifact = codegen.generate_artifact(
          &session.interner,
          &semantic.sir,
          type_view,
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

      let link_obj =
        codegen.generate(&session.interner, &semantic.sir, type_view);

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
  //
  // Re-copy is unconditional. A `(size, mtime)` skip
  // shortcut once left stale dylibs after a git
  // checkout where two builds landed in the same
  // minute with the same byte count but different
  // `LC_LOAD_DYLIB` layouts — dyld silently hangs in
  // that case.
  let candidates = [
    runtime_dir.join("deps").join(name),
    runtime_dir.join(name),
    runtime_dir.join("..").join("lib").join("vendor").join(name),
  ];

  for src in &candidates {
    if src.exists() {
      let _ = std::fs::copy(src, output_dir.join(name));
      return;
    }
  }
}
