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
use zo_dce::Dce;
use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_module_resolver::{ModuleResolver, extract_exports};
use zo_parser::{Parser, ParsingResult};
use zo_pp::PrettyPrinter;
use zo_profiler::Profiler;
use zo_reporter::{ErrorAggregator, Reporter, render_errors_to_stderr};
use zo_session::Session;
use zo_sir::Sir;
use zo_span::Span;
use zo_token::Token;
use zo_tokenizer::{TokenizationResult, Tokenizer};
use zo_tree::{NodeValue, Tree};
use zo_ty::Mutability;
use zo_value::ValueId;
use zo_value::{Local, LocalKind, Pubness};

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a [`Compiler`] instance.
pub struct Compiler {
  stats: Stats,
  profiler: Profiler,
  reporter: Reporter,
  module_resolver: ModuleResolver,
  /// The Guard against circular imports.
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

    // Auto-discover and compile lib.zo for pack validation.
    let declared_packs: Vec<(String, bool)> =
      if let Some(lib_path) = Self::discover_lib(file_path) {
        let lib_source = fs::read_to_string(&lib_path).unwrap_or_default();
        let lib_tokenization =
          Tokenizer::new(&lib_source, &mut session.interner).tokenize();
        let lib_parsing = Parser::new(&lib_tokenization, &lib_source).parse();
        let packs = Self::scan_packs(&lib_parsing.tree, &session.interner);

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
    let mut imported_enums: Vec<zo_module_resolver::ExportedEnum> = Vec::new();
    let mut module_sir_instructions = Vec::new();
    let mut module_next_value_id: u32 = 0;

    // --- Preload: auto-import std/io so showln etc.
    // are available without explicit `load io::showln;`.
    let preload = ["preload", "io", "assert", "math"];

    for module_name in preload {
      let sym = session.interner.intern(module_name);
      let preload_path = vec![sym];

      let resolved = self
        .module_resolver
        .resolve(&preload_path, &session.interner);

      if let Some(m) = resolved {
        let src = m.source.clone();
        let mod_tok = Tokenizer::new(&src, &mut session.interner).tokenize();
        let mod_par = Parser::new(&mod_tok, &src).parse();

        let mod_ana = Analyzer::new(
          &mod_par.tree,
          &mut session.interner,
          &mod_tok.literals,
          &mut session.ty_checker,
        );

        let mod_sem = mod_ana.analyze();

        let exports =
          extract_exports(mod_sem.sir, None, &session.interner, &mod_sem.funs);

        imported_funs.extend(exports.funs);

        for var in exports.vars {
          imported_vars.push(Local {
            name: var.name,
            ty_id: var.ty_id,
            value_id: var.init.unwrap_or(ValueId(0)),
            pubness: Pubness::Yes,
            mutability: Mutability::No,
            sir_value: var.init,
            local_kind: LocalKind::Variable,
          });
        }

        imported_enums.extend(exports.enums);

        module_sir_instructions.extend(exports.sir_instructions);

        module_next_value_id += exports.next_value_id;
      }
    }

    if !declared_packs.is_empty() {
      for module_path in &module_paths {
        if let Some(first_seg) = module_path.first() {
          let first_name = session.interner.get(*first_seg);
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
        let resolved =
          self.module_resolver.resolve(module_path, &session.interner);

        match resolved {
          Some(m) => {
            (m.source.clone(), m.selective_symbol.clone(), m.path.clone())
          }
          None => {
            let path_str = module_path
              .iter()
              .map(|s| session.interner.get(*s))
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

      let mod_tokenization =
        Tokenizer::new(&mod_source, &mut session.interner).tokenize();
      let mod_parsing = Parser::new(&mod_tokenization, &mod_source).parse();

      let mod_analyzer = Analyzer::new(
        &mod_parsing.tree,
        &mut session.interner,
        &mod_tokenization.literals,
        &mut session.ty_checker,
      );

      let mod_semantic = mod_analyzer.analyze();

      let exports = extract_exports(
        mod_semantic.sir,
        selective.as_deref(),
        &session.interner,
        &mod_semantic.funs,
      );

      imported_funs.extend(exports.funs);

      for var in exports.vars {
        imported_vars.push(Local {
          name: var.name,
          ty_id: var.ty_id,
          value_id: var.init.unwrap_or(ValueId(0)),
          pubness: Pubness::Yes,
          mutability: Mutability::No,
          sir_value: var.init,
          local_kind: LocalKind::Variable,
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
      &mut session.interner,
      &tokenization.literals,
      &mut session.ty_checker,
    );

    let has_imports = !imported_funs.is_empty()
      || !imported_vars.is_empty()
      || !imported_enums.is_empty();

    let analyzer = if has_imports {
      analyzer.with_imports(ImportedSymbols {
        funs: imported_funs,
        vars: imported_vars,
        enums: imported_enums,
      })
    } else {
      analyzer
    };

    let mut semantic = analyzer.analyze();
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

      Sir::offset_value_ids(&mut module_sir_instructions, main_next_vid);

      // Prepend: modules first, then main.
      let main_insns = std::mem::replace(
        &mut semantic.sir.instructions,
        module_sir_instructions,
      );

      semantic.sir.instructions.extend(main_insns);
      semantic.sir.next_value_id += module_next_value_id;
    }

    // Dead code elimination — find main by name.
    let main_sym = session.interner.intern("main");

    Dce::new(&mut semantic.sir, main_sym, &session.interner).eliminate();

    (semantic, tokenization, parsing, session)
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
      let (semantic, tokenization, parsing, session) =
        self.analyze_source(code, path);

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
        pp.format_sir(&semantic.sir, &session.interner);
        let sir_output = pp.finish();

        if let Err(error) = fs::write(&sir_path, sir_output) {
          eprintln!("Failed to write sir to {sir_path:?}: {error}");
        }
      }

      self.profiler.start_phase(CODEGEN_NAME);
      let codegen = Codegen::new(target);

      if should_emit_asm {
        let artifact =
          codegen.generate_artifact(&session.interner, &semantic.sir);
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

      codegen.generate(&session.interner, &semantic.sir, &output_path);

      self.stats.numartifacts += 1;

      self.profiler.end_phase(CODEGEN_NAME);
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
