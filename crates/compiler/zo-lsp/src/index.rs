use crate::position::LineIndex;

use zo_analyzer::SemanticResult;
use zo_compiler::{Compiler, default_core_search_paths};
use zo_interner::Symbol;
use zo_module_resolver::AbstractDef;
use zo_span::Span;
use zo_tree::Tree;
use zo_value::FunDef;

use rustc_hash::FxHashMap as HashMap;
use tower_lsp::lsp_types::Url;

use std::path::{Path, PathBuf};

/// Cached compilation state for a single open file.
pub struct FileState {
  pub line_index: LineIndex,
  pub tree: Tree,
  pub funs: Vec<FunDef>,
  pub abstract_defs: HashMap<Symbol, AbstractDef>,
  pub use_def_map: HashMap<Span, Span>,
  pub pack_paths: HashMap<Symbol, PathBuf>,
}

/// Per-workspace compilation cache.
pub struct SymbolIndex {
  pub files: HashMap<Url, FileState>,
}

impl SymbolIndex {
  pub fn new() -> Self {
    Self {
      files: HashMap::default(),
    }
  }

  /// Recompile a file and cache the results.
  pub fn update(&mut self, uri: &Url, source: &str, path: &Path) {
    let mut search_paths = default_core_search_paths();
    if let Some(parent) = path.parent() {
      search_paths.push(parent.to_path_buf());
    }

    log::info!(
      "update: path={} search_paths={:?}",
      path.display(),
      &search_paths,
    );

    let mut compiler = Compiler::with_search_paths(search_paths);

    let (semantic, _tokenization, parsing, _session) =
      compiler.analyze_source(source, path);

    log::info!(
      "update: funs={} pack_paths={} use_def_map={}",
      semantic.funs.len(),
      semantic.pack_paths.len(),
      semantic.use_def_map.len(),
    );

    let SemanticResult {
      funs,
      abstract_defs,
      use_def_map,
      pack_paths,
      ..
    } = semantic;

    let state = FileState {
      line_index: LineIndex::new(source),
      tree: parsing.tree,
      funs,
      abstract_defs,
      use_def_map,
      pack_paths,
    };

    self.files.insert(uri.clone(), state);
  }

  pub fn get(&self, uri: &Url) -> Option<&FileState> {
    self.files.get(uri)
  }
}
