use crate::index::{FileState, SymbolIndex};
use crate::position::LineIndex;

use zo_interner::Symbol;
use zo_span::Span;
use zo_token::Token;
use zo_tree::NodeValue;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
  DidChangeTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
  GotoDefinitionResponse, InitializeParams, InitializeResult,
  InitializedParams, Location, ServerCapabilities, ServerInfo,
  TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

use std::path::PathBuf;
use std::sync::Mutex;

pub struct ZoLanguageServer {
  client: Client,
  index: Mutex<SymbolIndex>,
}

impl ZoLanguageServer {
  pub fn new(client: Client) -> Self {
    Self {
      client,
      index: Mutex::new(SymbolIndex::new()),
    }
  }

  /// Find the smallest span in the tree containing `offset`.
  fn find_node_at_offset(
    state: &crate::index::FileState,
    offset: u32,
  ) -> Option<usize> {
    let mut best = None;
    let mut smallest_len = u16::MAX;

    for (idx, span) in state.tree.spans.iter().enumerate() {
      if offset >= span.start && offset < span.end() && span.len < smallest_len
      {
        smallest_len = span.len;
        best = Some(idx);
      }
    }

    best
  }
}

#[tower_lsp::async_trait]
impl LanguageServer for ZoLanguageServer {
  async fn initialize(
    &self,
    _params: InitializeParams,
  ) -> Result<InitializeResult> {
    Ok(InitializeResult {
      capabilities: ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
          TextDocumentSyncKind::FULL,
        )),
        definition_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
        ..Default::default()
      },
      server_info: Some(ServerInfo {
        name: "zo-lsp".into(),
        version: Some(env!("CARGO_PKG_VERSION").into()),
      }),
    })
  }

  async fn initialized(&self, _params: InitializedParams) {
    self
      .client
      .log_message(
        tower_lsp::lsp_types::MessageType::INFO,
        "zo-lsp initialized",
      )
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let source = params.text_document.text;

    let path = uri
      .to_file_path()
      .unwrap_or_else(|_| PathBuf::from(uri.path()));

    if let Ok(mut idx) = self.index.lock() {
      idx.update(&uri, &source, &path);
    }
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;

    let Some(change) = params.content_changes.into_iter().last() else {
      return;
    };

    let path = uri
      .to_file_path()
      .unwrap_or_else(|_| PathBuf::from(uri.path()));

    if let Ok(mut idx) = self.index.lock() {
      idx.update(&uri, &change.text, &path);
    }
  }

  async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
  ) -> Result<Option<GotoDefinitionResponse>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let idx = match self.index.lock() {
      Ok(g) => g,
      Err(_) => return Ok(None),
    };

    let Some(state) = idx.get(&uri) else {
      return Ok(None);
    };

    let offset = state.line_index.offset(pos.line, pos.character);

    let Some(node_idx) = Self::find_node_at_offset(state, offset) else {
      return Ok(None);
    };

    let use_span = state.tree.spans[node_idx];
    let token = state.tree.nodes[node_idx].token;
    log::info!(
      "goto_def: node_idx={} token={:?} span={}",
      node_idx,
      token,
      use_span,
    );

    // 1. Check the use-def map (locals + known functions).
    if let Some(&def_span) = state.use_def_map.get(&use_span) {
      let range = state.line_index.range(def_span);
      return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        range,
      ))));
    }

    // 2. Fallback: resolve ident by name against funs / abstracts.
    if state.tree.nodes[node_idx].token == Token::Ident
      && let Some(NodeValue::Symbol(sym)) = state.tree.value(node_idx as u32)
    {
      if let Some(loc) = resolve_fun(state, &uri, sym) {
        return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
      }

      if let Some(abs) = state.abstract_defs.get(&sym)
        && abs.span != Span::ZERO
      {
        let range = state.line_index.range(abs.span);
        return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
          uri.clone(),
          range,
        ))));
      }
    }

    Ok(None)
  }
}

/// Resolve a function symbol to an LSP Location.
/// Same-file defs use the cached LineIndex; cross-file
/// defs read the module source and build a temporary one.
fn resolve_fun(
  state: &FileState,
  current_uri: &Url,
  sym: Symbol,
) -> Option<Location> {
  let fun = state.funs.iter().find(|f| f.name == sym)?;

  if fun.span == Span::ZERO {
    return None;
  }

  match fun.owning_pack {
    None => {
      let range = state.line_index.range(fun.span);
      Some(Location::new(current_uri.clone(), range))
    }
    Some(pack) => {
      let path = state.pack_paths.get(&pack)?;
      let source = std::fs::read_to_string(path).ok()?;
      let line_index = LineIndex::new(&source);
      let range = line_index.range(fun.span);
      let file_uri = Url::from_file_path(path).ok()?;
      Some(Location::new(file_uri, range))
    }
  }
}
