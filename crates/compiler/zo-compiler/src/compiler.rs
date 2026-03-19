//! ```sh
//! cargo run --release --bin zon -- build zo-samples/tests/test_1000000_funcs.zo --target arm64-apple-darwin
//! ```

use crate::constants::{
  ANALYZER_NAME, CODEGEN_NAME, PARSER_NAME, TOKENIZER_NAME,
};

use crate::stage::Stage;

use zo_analyzer::{Analyzer, ImportedSymbols, SemanticResult};
use zo_codegen::codegen::Codegen;
use zo_codegen_backend::Target;
use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_module_resolver::{ModuleResolver, extract_exports};
use zo_parser::{Parser, ParsingResult};
use zo_pp::PrettyPrinter;
use zo_profiler::Profiler;
use zo_reporter::{ErrorAggregator, Reporter, render_errors_to_stderr};
use zo_span::Span;
use zo_token::Token;
use zo_tokenizer::{TokenizationResult, Tokenizer};
use zo_tree::{NodeValue, Tree};
use zo_ty_checker::TyChecker;
use zo_value::{Local, Mutability, Pubness, ValueId};

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a [`Compiler`] instance.
pub struct Compiler {
  stats: Stats,
  profiler: Profiler,
  reporter: Reporter,
  module_resolver: ModuleResolver,
  /// Guard against circular imports.
  compiling: HashSet<PathBuf>,
}
impl Compiler {
  /// Creates a new [`Compiler`] instance.
  pub fn new() -> Self {
    Self {
      stats: Stats::new(),
      profiler: Profiler::new(),
      reporter: Reporter::new(),
      module_resolver: ModuleResolver::new(Vec::new()),
      compiling: HashSet::new(),
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
      compiling: HashSet::new(),
    }
  }

  /// Scans a parse tree for `Token::Load` introducer nodes
  /// and extracts module paths from their Ident children.
  fn scan_loads(tree: &Tree) -> Vec<Vec<Symbol>> {
    let mut loads = Vec::new();

    for node in tree.nodes.iter() {
      if node.token != Token::Load || node.child_count == 0 {
        continue;
      }

      let mut path = Vec::new();

      for child_idx in node.children_range() {
        if let Some(child) = tree.nodes.get(child_idx)
          && child.token == Token::Ident
          && let Some(NodeValue::Symbol(sym)) = tree.value(child_idx as u32)
        {
          path.push(sym);
        }
      }

      if !path.is_empty() {
        loads.push(path);
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
    let nodes = &tree.nodes;

    for (i, node) in nodes.iter().enumerate() {
      if node.token != Token::Pack || node.child_count == 0 {
        continue;
      }

      // Check if previous node is Pub.
      let is_pub =
        i > 0 && nodes.get(i - 1).is_some_and(|n| n.token == Token::Pub);

      for child_idx in node.children_range() {
        if let Some(child) = nodes.get(child_idx)
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
  ) -> (SemanticResult, TokenizationResult, ParsingResult) {
    self.profiler.start_phase(TOKENIZER_NAME);
    let tokenizer = Tokenizer::new(source);
    let mut tokenization = tokenizer.tokenize();
    self.profiler.end_phase(TOKENIZER_NAME);

    self.profiler.start_phase(PARSER_NAME);
    let parser = Parser::new(&tokenization, source);
    let parsing = parser.parse();
    self.profiler.end_phase(PARSER_NAME);

    // Auto-discover and compile lib.zo for pack validation.
    let declared_packs: Vec<(String, bool)> =
      if let Some(lib_path) = Self::discover_lib(file_path) {
        let lib_source = fs::read_to_string(&lib_path).unwrap_or_default();
        let lib_tokenization = Tokenizer::new(&lib_source).tokenize();
        let lib_parsing = Parser::new(&lib_tokenization, &lib_source).parse();
        let packs =
          Self::scan_packs(&lib_parsing.tree, &lib_tokenization.interner);

        let dir = lib_path.parent().unwrap_or(Path::new("."));

        for (pack_name, _) in &packs {
          let pack_file = dir.join(format!("{pack_name}.zo"));
          let pack_dir = dir.join(pack_name).join("lib.zo");

          if !pack_file.is_file() && !pack_dir.is_file() {
            eprintln!(
              "Error: pack `{pack_name}` declared in lib.zo \
               but `{pack_name}.zo` not found"
            );
          }
        }

        packs
      } else {
        Vec::new()
      };

    // Resolve and compile loaded modules BEFORE analysis.
    let module_paths = Self::scan_loads(&parsing.tree);
    let mut imported_funs = Vec::new();
    let mut imported_vars = Vec::new();
    let mut module_sir_instructions = Vec::new();
    let mut module_next_value_id: u32 = 0;

    if !declared_packs.is_empty() {
      for module_path in &module_paths {
        if let Some(first_seg) = module_path.first() {
          let first_name = tokenization.interner.get(*first_seg);
          let is_declared =
            declared_packs.iter().any(|(name, _)| name == first_name);

          if !is_declared {
            eprintln!(
              "Warning: module `{first_name}` \
               not declared in lib.zo"
            );
          }
        }
      }
    }

    for module_path in &module_paths {
      let (mod_source, selective, resolved_path) = {
        let resolved = self
          .module_resolver
          .resolve(module_path, &tokenization.interner);

        match resolved {
          Some(m) => {
            (m.source.clone(), m.selective_symbol.clone(), m.path.clone())
          }
          None => {
            let path_str = module_path
              .iter()
              .map(|s| tokenization.interner.get(*s))
              .collect::<Vec<_>>();

            eprintln!("Error: unresolved module `{}`", path_str.join("::"));

            continue;
          }
        }
      };

      if self.compiling.contains(&resolved_path) {
        eprintln!("Error: circular import detected");
        continue;
      }

      self.compiling.insert(resolved_path.clone());

      let mod_tokenization = Tokenizer::new(&mod_source).tokenize();
      let mod_parsing = Parser::new(&mod_tokenization, &mod_source).parse();
      let mod_analyzer = Analyzer::new(
        &mod_parsing.tree,
        &mod_tokenization.interner,
        &mod_tokenization.literals,
      );
      let mod_semantic = mod_analyzer.analyze();

      let mut dst_ty_checker = TyChecker::new();

      let exports = extract_exports(
        mod_semantic.sir,
        selective.as_deref(),
        &mod_tokenization.interner,
        &mut tokenization.interner,
        &mod_semantic.ty_checker,
        &mut dst_ty_checker,
      );

      imported_funs.extend(exports.funs);

      for var in exports.vars {
        imported_vars.push(Local {
          name: var.name,
          ty_id: var.ty_id,
          value_id: var.init.unwrap_or(ValueId(0)),
          pubness: Pubness::Yes,
          mutability: Mutability::No,
        });
      }

      module_sir_instructions.extend(exports.sir_instructions);
      module_next_value_id += exports.next_value_id;

      self.compiling.remove(&resolved_path);
    }

    // Analyze with imported symbols pre-loaded.
    self.profiler.start_phase(ANALYZER_NAME);

    let analyzer = Analyzer::new(
      &parsing.tree,
      &tokenization.interner,
      &tokenization.literals,
    );

    let analyzer = if !imported_funs.is_empty() || !imported_vars.is_empty() {
      analyzer.with_imports(ImportedSymbols {
        funs: imported_funs,
        vars: imported_vars,
      })
    } else {
      analyzer
    };

    let mut semantic = analyzer.analyze();
    self.profiler.end_phase(ANALYZER_NAME);

    // Merge module SIR before main SIR for codegen.
    if !module_sir_instructions.is_empty() {
      offset_value_ids(&mut semantic.sir.instructions, module_next_value_id);
      semantic.sir.next_value_id += module_next_value_id;

      let mut merged = module_sir_instructions;
      merged.append(&mut semantic.sir.instructions);
      semantic.sir.instructions = merged;
    }

    // Dead code elimination.
    zo_dce::eliminate_dead_functions(&mut semantic.sir);

    (semantic, tokenization, parsing)
  }

  /// Compiles a collections of files based on the [`Target`].
  pub fn compile(
    &mut self,
    files: &[(&PathBuf, String)],
    target: Target,
    stages: &[Stage],
    output_path: &Option<PathBuf>,
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

    for (path, code) in files.iter() {
      let (semantic, tokenization, parsing) = self.analyze_source(code, path);

      self.stats.numtokens += tokenization.tokens.len();
      self.stats.numnodes += parsing.tree.nodes.len();
      self.stats.numinferences += semantic.annotations.len();

      if should_emit_tokens {
        let tokens_path = path.with_extension("tokens");
        let mut pp = PrettyPrinter::new();
        pp.format_tokens(&tokenization.tokens, code);
        let tokens_output = pp.finish();

        if let Err(error) = fs::write(&tokens_path, tokens_output) {
          eprintln!("Failed to write tokens to {tokens_path:?}: {error}");
        }
      }

      if should_emit_tree {
        let tree_path = path.with_extension("tree");
        let mut pp = PrettyPrinter::new();
        pp.format_tree(&parsing.tree, code);
        let tree_output = pp.finish();

        if let Err(error) = fs::write(&tree_path, tree_output) {
          eprintln!("Failed to write tree to {tree_path:?}: {error}");
        }
      }

      if should_emit_sir {
        let sir_path = path.with_extension("sir");
        let mut pp = PrettyPrinter::new();
        pp.format_sir(&semantic.sir, &tokenization.interner);
        let sir_output = pp.finish();

        if let Err(error) = fs::write(&sir_path, sir_output) {
          eprintln!("Failed to write sir to {sir_path:?}: {error}");
        }
      }

      self.profiler.start_phase(CODEGEN_NAME);
      let codegen = Codegen::new(target);

      if should_emit_asm {
        let artifact =
          codegen.generate_artifact(&tokenization.interner, &semantic.sir);
        let asm_path = path.with_extension("asm");

        let mut pp = PrettyPrinter::new();
        pp.format_asm(&artifact, target);
        let asm_output = pp.finish();

        if let Err(error) = fs::write(&asm_path, asm_output) {
          eprintln!("Failed to write assembly to {asm_path:?}: {error}");
        }
      }

      let output_path = match &output_path {
        Some(p) => p.clone(),
        None => path.with_extension(""),
      };

      codegen.generate(&tokenization.interner, &semantic.sir, &output_path);

      self.stats.numartifacts += 1;

      self.profiler.end_phase(CODEGEN_NAME);
      self.profiler.set_output(path.display().to_string());
    }

    let errors = self.reporter.errors();

    if !errors.is_empty() {
      let mut aggregator = ErrorAggregator::new();

      aggregator.add_errors(errors);

      for (path, source) in files.iter() {
        let filename = path.to_string_lossy();
        let _ = render_errors_to_stderr(&aggregator, source, &filename);
      }

      aggregator.clear();

      return Err(Error::new(ErrorKind::InternalCompilerError, Span::ZERO));
    }

    self.profiler.set_tokens_count(self.stats.numtokens);
    self.profiler.set_nodes_count(self.stats.numnodes);
    self.profiler.set_inferences_count(self.stats.numinferences);
    self.profiler.set_artifacts_count(self.stats.numartifacts);
    self.profiler.set_artifacts_linked(self.stats.numartifacts);
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
  /// The number of artifacts.
  numartifacts: usize,
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
    }
  }
}

/// Offsets all ValueIds in SIR instructions by `offset`.
/// Used when prepending module SIR to avoid ID collisions.
fn offset_value_ids(instructions: &mut [zo_sir::Insn], offset: u32) {
  use zo_sir::Insn;
  use zo_value::ValueId;

  let off = |v: &mut ValueId| v.0 += offset;

  for insn in instructions.iter_mut() {
    match insn {
      Insn::ConstInt { .. }
      | Insn::ConstFloat { .. }
      | Insn::ConstBool { .. }
      | Insn::ConstString { .. }
      | Insn::ModuleLoad { .. }
      | Insn::PackDecl { .. }
      | Insn::Label { .. }
      | Insn::Jump { .. } => {}
      Insn::VarDef { init, .. } => {
        if let Some(v) = init {
          off(v);
        }
      }
      Insn::Store { value, .. } => off(value),
      Insn::FunDef { .. } => {}
      Insn::Return { value, .. } => {
        if let Some(v) = value {
          off(v);
        }
      }
      Insn::Call { args, .. } => {
        for a in args.iter_mut() {
          off(a);
        }
      }
      Insn::Load { dst, .. } => off(dst),
      Insn::BinOp { dst, lhs, rhs, .. } => {
        off(dst);
        off(lhs);
        off(rhs);
      }
      Insn::UnOp { rhs, .. } => off(rhs),
      Insn::BranchIfNot { cond, .. } => off(cond),
      Insn::Directive { value, .. } => off(value),
      Insn::Template { id, .. } => off(id),
    }
  }
}
