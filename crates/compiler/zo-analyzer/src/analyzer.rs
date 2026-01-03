use zo_executor::Executor;
use zo_interner::Interner;
use zo_sir::Sir;
use zo_token::LiteralStore;
use zo_tree::Tree;
use zo_ty::Annotation;

/// Represents the result of semantic analysis.
pub struct SemanticResult {
  /// The semantic intermediate representation [`Sir`].
  pub sir: Sir,
  /// The collections of types [`Annotation`].
  pub annotations: Vec<Annotation>,
}

/// Represents the [`Analyzer`] phase.
pub struct Analyzer<'a> {
  /// The reference of parse [`Tree`].
  tree: &'a Tree,
  /// The reference of a string [`Interner`].
  interner: &'a Interner,
  /// The reference of a [`LiteralStore`].
  literals: &'a LiteralStore,
}

impl<'a> Analyzer<'a> {
  /// Creates a new [`Analyzer`] instance.
  pub const fn new(
    tree: &'a Tree,
    interner: &'a Interner,
    literals: &'a LiteralStore,
  ) -> Self {
    Self {
      tree,
      interner,
      literals,
    }
  }

  /// Analyzes a parse [`Tree`] to build semantic IR.
  pub fn analyze(self) -> SemanticResult {
    let executor = Executor::new(self.tree, self.interner, self.literals);
    let (sir, annotations) = executor.execute();

    SemanticResult { sir, annotations }
  }
}
