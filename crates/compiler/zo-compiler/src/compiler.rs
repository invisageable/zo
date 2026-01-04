//! ```sh
//! cargo run --release --bin zon -- build zo-samples/tests/test_1000000_funcs.zo --target arm64-apple-darwin
//! ```

use crate::constants::{
  ANALYZER_NAME, CODEGEN_NAME, PARSER_NAME, TOKENIZER_NAME,
};
use crate::stage::Stage;

use zo_analyzer::Analyzer;
use zo_codegen::codegen::Codegen;
use zo_codegen_backend::Target;
use zo_error::{Error, ErrorKind};
use zo_parser::Parser;
use zo_pp::PrettyPrinter;
use zo_profiler::Profiler;
use zo_reporter::{ErrorAggregator, Reporter, render_errors_to_stderr};
use zo_span::Span;
use zo_tokenizer::Tokenizer;

use std::fs;
use std::path::PathBuf;

/// Represents a [`Compiler`] instance.
pub struct Compiler {
  stats: Stats,
  profiler: Profiler,
  reporter: Reporter,
}
impl Compiler {
  /// Creates a new [`Compiler`] instance.
  pub fn new() -> Self {
    Self {
      stats: Stats::new(),
      profiler: Profiler::new(),
      reporter: Reporter::new(),
    }
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
      self.profiler.start_phase(TOKENIZER_NAME);
      let tokenizer = Tokenizer::new(code);
      let tokenization = tokenizer.tokenize();
      self.stats.numtokens += tokenization.tokens.len();
      self.profiler.end_phase(TOKENIZER_NAME);

      if should_emit_tokens {
        let tokens_path = path.with_extension("tokens");
        let mut pp = PrettyPrinter::new();
        pp.format_tokens(&tokenization.tokens, code);
        let tokens_output = pp.finish();

        if let Err(error) = fs::write(&tokens_path, tokens_output) {
          eprintln!("Failed to write tokens to {tokens_path:?}: {error}");
        }
      }

      self.profiler.start_phase(PARSER_NAME);
      let parser = Parser::new(&tokenization, code);
      let parsing = parser.parse();
      self.stats.numnodes += parsing.tree.nodes.len();
      self.profiler.end_phase(PARSER_NAME);

      if should_emit_tree {
        let tree_path = path.with_extension("tree");
        let mut pp = PrettyPrinter::new();
        pp.format_tree(&parsing.tree, code);
        let tree_output = pp.finish();

        if let Err(error) = fs::write(&tree_path, tree_output) {
          eprintln!("Failed to write tree to {tree_path:?}: {error}");
        }
      }

      self.profiler.start_phase(ANALYZER_NAME);
      let analyzer = Analyzer::new(
        &parsing.tree,
        &tokenization.interner,
        &tokenization.literals,
      );
      let semantic = analyzer.analyze();
      self.stats.numinferences += semantic.annotations.len();
      self.profiler.end_phase(ANALYZER_NAME);

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
