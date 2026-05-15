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
  ErrorAggregator, Reporter, render_errors_to_stderr, report_error,
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
/// 1. `ZO_STD_PATH` env var — explicit override.
/// 2. `<exe-dir>/../lib/std` — installed layout.
/// 3. `<exe-dir>/../../crates/compiler-lib/std` — dev layout
///    (works for both `target/debug/zo` and
///    `target/debug/fret`, which sit at the same depth).
///
/// Returns an empty `Vec` if none of these resolve, in which
/// case preload silently no-ops and `showln` etc. surface
/// as `Undefined variable` — matches the old behavior so
/// callers needing a non-default layout can still pass
/// `with_search_paths`.
pub fn default_std_search_paths() -> Vec<PathBuf> {
  if let Ok(std_path) = env::var("ZO_STD_PATH") {
    return vec![PathBuf::from(std_path)];
  }

  zo_host_paths::first_existing_lib_dir("std")
    .map(|p| vec![p])
    .unwrap_or_default()
}

/// One `load` statement: a fully-qualified module path
/// (`std::io::*` → `[std, io]`) plus the span of the `load`
/// node in the source for diagnostics. Worklist entry for
/// the transitive-load closure.
pub type LoadRef = zo_span::Spanned<Vec<Symbol>>;

/// File-as-pack rule: returns the implicit pack name for a
/// loaded module file, or `None` when the file is a package
/// manifest (`lib.zo`) or a binary entry (`main.zo`). The
/// pack name is the file's stem; the analyzer synthesizes a
/// `pack <name>;` decl before walking the tree so items in
/// `std/math.zo` namespace as `math::*` without an explicit
/// `pack math;` line.
fn implicit_pack_for(path: &Path) -> Option<&str> {
  let stem = path.file_stem().and_then(|s| s.to_str())?;

  match stem {
    "lib" | "main" => None,
    other => Some(other),
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
      module_resolver: ModuleResolver::new(default_std_search_paths()),
      compiling: HashSet::default(),
      module_table: HashMap::default(),
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
    }
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

    for (i, node) in tree.nodes_with_token(Token::Pack) {
      if node.child_count == 0 {
        continue;
      }

      let is_pub = tree.is_pub_at(i);

      for child_idx in node.children_range() {
        if let Some(child) = tree.nodes.get(child_idx)
          && child.token == Token::Ident
          && let Some(NodeValue::Symbol(sym)) = tree.value(child_idx as u32)
        {
          packs.push((interner.get(sym).to_string(), is_pub));
          break;
        }
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

    // Auto-discover lib.zo and compile declared packs.
    // Each `pub pack foo;` in lib.zo compiles foo.zo and
    // stores its exports in the module_table. User `load`
    // statements will query this table.
    let has_lib_zo = if let Some(lib_path) = Self::discover_lib(file_path) {
      let lib_source = fs::read_to_string(&lib_path).unwrap_or_default();
      let lib_tokenization =
        Tokenizer::new(&lib_source, &mut session.interner).tokenize();
      let lib_parsing = Parser::new(&lib_tokenization, &lib_source).parse();
      let packs = Self::scan_packs(&lib_parsing.tree, &session.interner);

      let dir = lib_path.parent().unwrap_or(Path::new("."));

      for (pack_name, is_pub) in &packs {
        let pack_file = dir.join(format!("{pack_name}.zo"));
        let pack_dir = dir.join(pack_name).join("lib.zo");

        let pack_path = if pack_file.is_file() {
          Some(pack_file)
        } else if pack_dir.is_file() {
          Some(pack_dir)
        } else {
          report_error(Error::new(
            ErrorKind::PackFileNotFound,
            Span { start: 0, len: 0 },
          ));

          None
        };

        if let Some(path) = pack_path
          && *is_pub
        {
          let pack_source = fs::read_to_string(&path).unwrap_or_default();
          let pack_tok =
            Tokenizer::new(&pack_source, &mut session.interner).tokenize();
          let pack_par = Parser::new(&pack_tok, &pack_source).parse();

          let implicit_sym =
            implicit_pack_for(&path).map(|stem| session.interner.intern(stem));

          let pack_sem = Analyzer::new(
            &pack_par.tree,
            &mut session.interner,
            &pack_tok.literals,
            &mut session.ty_checker,
          )
          .with_config(AnalyzerConfig {
            implicit_pack: implicit_sym,
            ..AnalyzerConfig::default()
          })
          .analyze();

          let exports = extract_exports(
            pack_sem.sir,
            None,
            &session.interner,
            &pack_sem.funs,
          );

          let sym = session.interner.intern(pack_name);

          self.module_table.insert(sym, exports);
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
    let std_sym = session.interner.intern("std");
    // Cumulative imports — every loaded module sees what
    // earlier modules exported (preload's `Option`/`Result`
    // enums, std::int's primitive methods, etc.) and
    // contributes its own exports. Cloned per loaded-module
    // analyzer construction; ownership flows into the final
    // user analyzer at the end.
    let mut imports = ImportedSymbols {
      funs: Vec::new(),
      vars: Vec::new(),
      enums: Vec::new(),
      abstract_defs: HashMap::default(),
    };

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
    // imported" surface — its top-level `load std::…::*;`
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

        let implicit_sym = implicit_pack_for(&resolved_path)
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

        imports.funs.extend(exports.funs);

        for var in exports.vars {
          imports.vars.push(Local {
            name: var.name,
            ty_id: var.ty_id,
            value_id: var.init.unwrap_or(ValueId(0)),
            pubness: Pubness::Yes,
            mutability: Mutability::No,
            sir_value: var.init,
            local_kind: LocalKind::Variable,
          });
        }

        imports.enums.extend(exports.enums);
        imports.abstract_defs.extend(mod_sem.abstract_defs);
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

      // Check module_table first (populated from lib.zo packs).
      if let Some(mut exports) = self.module_table.remove(&first_seg) {
        Sir::offset_labels(&mut exports.sir_instructions, module_next_label_id);

        imports.funs.extend(exports.funs);

        for var in exports.vars {
          imports.vars.push(Local {
            name: var.name,
            ty_id: var.ty_id,
            value_id: var.init.unwrap_or(ValueId(0)),
            pubness: Pubness::Yes,
            mutability: Mutability::No,
            sir_value: var.init,
            local_kind: LocalKind::Variable,
          });
        }

        imports.enums.extend(exports.enums);
        module_sir_instructions.extend(exports.sir_instructions);

        module_next_value_id += exports.next_value_id;
        module_next_label_id += exports.next_label_id;

        continue;
      }

      // lib.zo exists but module not declared — error.
      // `std::…` paths are exempt: the user's project lib.zo
      // governs THEIR subpackage layout, not what std exports.
      // Without this skip, a project with `pack foo;` in
      // lib.zo would reject preload's `load std::io::*;` and
      // every other transitive std import.
      if has_lib_zo && first_seg != std_sym {
        report_error(Error::new(ErrorKind::ModuleNotDeclared, load_span));

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

      let implicit_sym = implicit_pack_for(&resolved_path)
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

      Sir::offset_labels(&mut exports.sir_instructions, module_next_label_id);

      imports.funs.extend(exports.funs);

      for var in exports.vars {
        imports.vars.push(Local {
          name: var.name,
          ty_id: var.ty_id,
          value_id: var.init.unwrap_or(ValueId(0)),
          pubness: Pubness::Yes,
          mutability: Mutability::No,
          sir_value: var.init,
          local_kind: LocalKind::Variable,
        });
      }

      imports.enums.extend(exports.enums);
      imports.abstract_defs.extend(mod_semantic.abstract_defs);
      module_sir_instructions.extend(exports.sir_instructions);
      module_next_value_id += exports.next_value_id;
      module_next_label_id += exports.next_label_id;

      self.compiling.remove(&resolved_path);
    }

    // Analyze with imported symbols pre-loaded.
    self.profiler.start_phase(ANALYZER_NAME);

    let analyzer = Analyzer::new(
      &parsing.tree,
      &mut session.interner,
      &tokenization.literals,
      &mut session.ty_checker,
    );

    // The leaf of each transitively-loaded path IS the pack
    // name (`std::io::*` → `io`). Surfaced so the user
    // analyzer's qualified-call resolution finds them.
    let in_scope_packs: Vec<Symbol> = module_paths
      .iter()
      .filter_map(|m| m.value.last().copied())
      .collect();

    // The user file's `<img src="…">` and similar
    // path-typed attributes are resolved against this
    // directory at attribute-build time — so the
    // compiled binary holds absolute paths and renders
    // assets regardless of CWD at run time.
    let mut semantic = analyzer
      .with_config(AnalyzerConfig {
        imports,
        source_dir: file_path.parent().map(Path::to_path_buf),
        implicit_pack: None,
        in_scope_packs,
      })
      .analyze();
    self.profiler.end_phase(ANALYZER_NAME);

    // Drain thread-local errors into the compiler reporter.
    let tl_errors = zo_reporter::collect_errors();
    if !tl_errors.is_empty() {
      self.reporter.collect_errors(&tl_errors);
    }

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

        let _ = render_errors_to_stderr(&aggregator, source, &filename);
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
    self.profiler.summary(target.name());

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
