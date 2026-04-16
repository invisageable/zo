use zo_executor::{AbstractDef, Executor};
use zo_interner::{Interner, Symbol};
use zo_module_resolver::ExportedEnum;
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
  /// Function definitions from the executor (carries
  /// return_type_args for ext functions).
  pub funs: Vec<FunDef>,
  /// Abstract definitions from the executor.
  pub abstract_defs: std::collections::HashMap<Symbol, AbstractDef>,
}

/// Imported module symbols to pre-load into the executor.
pub struct ImportedSymbols {
  /// The Function definitions from loaded modules.
  pub funs: Vec<FunDef>,
  /// Constants from loaded modules.
  pub vars: Vec<Local>,
  /// Enum definitions from loaded modules (raw variant data
  /// for re-interning in the executor's own TyChecker).
  pub enums: Vec<ExportedEnum>,
  /// Abstract definitions from loaded modules.
  pub abstract_defs: std::collections::HashMap<Symbol, AbstractDef>,
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
  /// Imported symbols from loaded modules.
  imports: Option<ImportedSymbols>,
}

impl<'a> Analyzer<'a> {
  /// Creates a new [`Analyzer`] instance.
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
    let mut executor =
      Executor::new(self.tree, self.interner, self.literals, self.ty_checker);

    if let Some(imports) = self.imports {
      executor = executor.with_imports(
        imports.funs,
        imports.vars,
        imports.enums,
        imports.abstract_defs,
      );
    }

    let (sir, annotations, funs, abstract_defs) = executor.execute();

    SemanticResult {
      sir,
      annotations,
      abstract_defs,
      funs,
    }
  }
}
