use zo_error::{Error, ErrorKind};
use zo_executor::Executor;
use zo_interner::{Interner, Symbol};
use zo_module_resolver::{
  AbstractDef, AbstractImpl, ExportedGenericBody, ImportedSymbols,
};
use zo_reporter::{error_count, report_error};
use zo_sir::Sir;
use zo_span::Span;
use zo_token::{LiteralStore, Token};
use zo_tree::Tree;
use zo_ty::Annotation;
use zo_ty_checker::TyChecker;
use zo_value::FunDef;

use rustc_hash::FxHashMap as HashMap;

use std::path::PathBuf;

/// Represents the result of semantic analysis.
pub struct SemanticResult {
  /// The semantic intermediate representation [`Sir`].
  pub sir: Sir,
  /// The collections of types [`Annotation`].
  pub annotations: Vec<Annotation>,
  /// Function definitions from the executor (carries
  /// return_type_args for ext functions).
  pub funs: Vec<FunDef>,
  /// Abstract definitions from the executor.
  pub abstract_defs: HashMap<Symbol, AbstractDef>,
  /// `(Abstract, Type) -> AbstractImpl` registrations from
  /// every `apply Abstract for Type { ... }` block the
  /// analyzer walked. Hands them through to the compiler
  /// driver so cross-module dispatch (`a == b` calling
  /// the imported `Type::eq` instead of falling back to a
  /// primitive comparison) works end-to-end.
  pub abstract_impls: HashMap<(Symbol, Symbol), AbstractImpl>,
  /// Generic apply-block bodies recorded during this run —
  /// the importer splices each into its own tree to make
  /// `arr_$::method` instantiations re-executable across
  /// module boundaries (PLAN_CROSS_MODULE_GENERICS).
  pub generic_bodies: Vec<ExportedGenericBody>,
  /// Maps each identifier use-span to its definition-span.
  pub use_def_map: HashMap<Span, Span>,
  /// Pack symbol → absolute source path for every compiled
  /// module. Populated by the compiler during module
  /// resolution so the LSP can resolve cross-file jumps.
  pub pack_paths: HashMap<Symbol, std::path::PathBuf>,
}

/// Per-call analyzer configuration. Bundles every optional
/// piece of state the analyzer (and through it the
/// executor) needs at construction time. Replaces the
/// four-setter forwarder pattern — each call site builds
/// one of these instead of chaining `with_imports`,
/// `with_source_dir`, `with_implicit_pack`, and
/// `with_in_scope_packs`.
#[derive(Default)]
pub struct AnalyzerConfig {
  /// Function/var/enum/abstract definitions inherited
  /// from previously-compiled modules.
  pub imports: ImportedSymbols,
  /// Directory of the source file. Path-typed template
  /// attributes (`<img src="…">`) are resolved against it
  /// so the compiled output holds CWD-independent paths.
  /// `None` for preload packs / module imports — their
  /// templates never reach the renderer.
  pub source_dir: Option<PathBuf>,
  /// Absolute path of the source file. Stamped into each
  /// `AbstractImpl` so the duplicate-impl diagnostic can
  /// render both colliding modules by name. `None` for
  /// in-memory fragments (`html_inline`) where no on-disk
  /// path exists.
  pub source_path: Option<PathBuf>,
  /// Implicit pack name derived from the file basename
  /// (file-as-pack rule). `None` for entry files
  /// (`lib.zo`, `main.zo`) and library manifests.
  pub implicit_pack: Option<Symbol>,
  /// Packs already merged into the SIR via preload +
  /// transitive loads. Seeded into the executor so
  /// qualified call sites (`io::showln(...)`) resolve
  /// without an explicit `load core::io;` in the user file.
  pub in_scope_packs: Vec<Symbol>,
  /// `true` when this analyzer call is for the user's
  /// entry file — the path the driver was invoked on.
  /// Gates the `fun main` entry-point check: only the
  /// entry file is required to declare `main`. Modules
  /// loaded transitively (preload, `core::*`, user packs)
  /// stay default `false` and skip the check.
  pub is_entry: bool,
  /// `true` when the test harness synthesizes the entry
  /// point (`zo test`). Suppresses the missing-`main` check
  /// for the entry file: a library or test file need not
  /// declare `main`, since `compile_test` provides one.
  pub test_mode: bool,
  /// Index into the compiler's file table. Stamped onto
  /// every `Error` the executor emits so the renderer
  /// resolves spans against the correct source text.
  /// `0` = entry file. `0xFFFF` = unset (default).
  pub file_id: u16,
}

/// Represents the [`Analyzer`] phase.
pub struct Analyzer<'a> {
  /// The reference of parse [`Tree`].
  tree: &'a Tree,
  /// The reference of a string [`Interner`].
  interner: &'a mut Interner,
  /// The reference of a [`LiteralStore`].
  literals: &'a LiteralStore,
  /// The type checker instance (borrowed from caller).
  ty_checker: &'a mut TyChecker,
  /// All configurable inputs bundled into one struct.
  config: AnalyzerConfig,
}

impl<'a> Analyzer<'a> {
  /// Creates a new [`Analyzer`] instance with default
  /// config (no imports, no source dir, no implicit pack,
  /// no in-scope packs). Use [`Analyzer::with_config`] to
  /// supply non-defaults.
  pub fn new(
    tree: &'a Tree,
    interner: &'a mut Interner,
    literals: &'a LiteralStore,
    ty_checker: &'a mut TyChecker,
  ) -> Self {
    Self {
      tree,
      interner,
      literals,
      ty_checker,
      config: AnalyzerConfig::default(),
    }
  }

  /// Applies every non-default field of `config` to the
  /// underlying executor. Single point of forwarding for
  /// all per-call configuration — adding a new option
  /// touches only [`AnalyzerConfig`] and the matching
  /// `Executor::with_*` setter, never this `Analyzer` body.
  pub fn with_config(mut self, config: AnalyzerConfig) -> Self {
    self.config = config;
    self
  }

  fn has_main(&mut self) -> bool {
    let main_sym = self.interner.intern("main");

    self
      .tree
      .nodes_with_token(Token::Fun)
      .any(|(idx, _)| self.tree.first_ident_child_symbol(idx) == Some(main_sym))
  }

  /// Analyzes a parse [`Tree`] to build semantic IR.
  pub fn analyze(mut self) -> SemanticResult {
    // Upstream tokenizer/parser errors can erase `fun main`
    // from the tree (unterminated comments, mismatched
    // delimiters). Suppress the missing-main check then —
    // the primary diagnostic wins. Matches rustc E0601.
    //
    // Under `zo test` the harness synthesizes `main`, so a
    // library/test entry need not declare one — skip too.
    if self.config.is_entry
      && !self.config.test_mode
      && error_count() == 0
      && !self.has_main()
    {
      report_error(Error::new(
        ErrorKind::MissingMainFunction,
        self.tree.eof_span(),
      ));

      // TODO: derive default to `SemanticResult` and return `SemanticResult::default()`
      return SemanticResult {
        sir: Sir::new(),
        annotations: Vec::new(),
        funs: Vec::new(),
        abstract_defs: HashMap::default(),
        abstract_impls: HashMap::default(),
        generic_bodies: Vec::new(),
        use_def_map: HashMap::default(),
        pack_paths: HashMap::default(),
      };
    }

    let mut executor =
      Executor::new(self.tree, self.interner, self.literals, self.ty_checker);

    let AnalyzerConfig {
      imports,
      source_dir,
      source_path,
      implicit_pack,
      in_scope_packs,
      is_entry: _,
      test_mode: _,
      file_id,
    } = self.config;

    if !imports.is_empty() {
      executor = executor.with_imports(imports);
    }

    if let Some(dir) = source_dir {
      executor = executor.with_source_dir(dir);
    }

    if let Some(path) = source_path {
      executor = executor.with_source_path(path);
    }

    if let Some(name) = implicit_pack {
      executor = executor.with_implicit_pack(name);
    }

    if !in_scope_packs.is_empty() {
      executor = executor.with_in_scope_packs(in_scope_packs);
    }

    executor = executor.with_file_id(file_id);

    let (
      sir,
      annotations,
      funs,
      abstract_defs,
      abstract_impls,
      generic_bodies,
      use_def_map,
    ) = executor.execute();

    SemanticResult {
      sir,
      annotations,
      abstract_defs,
      abstract_impls,
      funs,
      generic_bodies,
      use_def_map,
      pack_paths: HashMap::default(),
    }
  }
}
