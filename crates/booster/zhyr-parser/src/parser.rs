#![allow(dead_code)]

use zhyr_ast::ast::Ast;
use zhyr_parser_js::parser as js;
use zhyr_parser_py::parser as py;

use zhoo_session::backend::BackendKind;
use zhoo_session::session::Session;

use zo_core::Result;

struct Parser;

impl Parser {
  fn parse(
    &mut self,
    session: &mut Session,
    paths: &Vec<std::path::PathBuf>,
  ) -> Result<Ast> {
    match &session.settings.backend.kind {
      BackendKind::Js => js::parse(session, paths),
      BackendKind::Py => py::parse(session, paths),
      _ => panic!(),
    }
  }
}

/// ## examples.
///
/// ```
/// ```
pub fn parse(
  session: &mut Session,
  paths: &Vec<std::path::PathBuf>,
) -> Result<Ast> {
  Parser.parse(session, paths)
}
