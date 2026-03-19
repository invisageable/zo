use zo_executor::Executor;
use zo_interner::Interner;
use zo_sir::Sir;
use zo_token::LiteralStore;
use zo_tree::Tree;
use zo_ty::Annotation;
use zo_ty_checker::TyChecker;
use zo_value::{FunDef, Local};

/// Represents the result of semantic analysis.
pub struct SemanticResult {
  /// The semantic intermediate representation [`Sir`].
  pub sir: Sir,
  /// The collections of types [`Annotation`].
  pub annotations: Vec<Annotation>,
  /// The type checker state for cross-module translation.
  pub ty_checker: TyChecker,
}

/// Imported module symbols to pre-load into the executor.
pub struct ImportedSymbols {
  /// The Function definitions from loaded modules.
  pub funs: Vec<FunDef>,
  /// Constants from loaded modules.
  pub vars: Vec<Local>,
}

/// Represents the [`Analyzer`] phase.
pub struct Analyzer<'a> {
  /// The reference of parse [`Tree`].
  tree: &'a Tree,
  /// The reference of a string [`Interner`].
  interner: &'a Interner,
  /// The reference of a [`LiteralStore`].
  literals: &'a LiteralStore,
  /// Imported symbols from loaded modules.
  imports: Option<ImportedSymbols>,
}

impl<'a> Analyzer<'a> {
  /// Creates a new [`Analyzer`] instance.
  pub fn new(
    tree: &'a Tree,
    interner: &'a Interner,
    literals: &'a LiteralStore,
  ) -> Self {
    Self {
      tree,
      interner,
      literals,
      imports: None,
    }
  }

  /// Sets imported symbols from loaded modules.
  pub fn with_imports(mut self, imports: ImportedSymbols) -> Self {
    self.imports = Some(imports);
    self
  }

  /// Analyzes a parse [`Tree`] to build semantic IR.
  pub fn analyze(self) -> SemanticResult {
    let mut executor = Executor::new(self.tree, self.interner, self.literals);

    if let Some(imports) = self.imports {
      executor = executor.with_imports(imports.funs, imports.vars);
    }

    let (sir, annotations, ty_checker) = executor.execute();

    SemanticResult {
      sir,
      annotations,
      ty_checker,
    }
  }
}
