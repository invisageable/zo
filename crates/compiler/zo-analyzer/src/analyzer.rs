// note #1 — The analyzer must be able to do type inference.
// It's the better approach for writing programs.
//
// There is some useful things related to what we should do from the
// Pony language. We need to investigage about it and read papers by Sylvan
// Clebsch supervised by Sophia Drossopoulou.
//
// * https://www.ponylang.io/media/papers/fast-cheap.pdf.
// * https://www.ponylang.io/media/papers/OGC.pdf.
// * https://www.ponylang.io/media/papers/opsla237-clebsch.pdf.
// * https://www.ponylang.io/media/papers/a_string_of_ponies.pdf.
// * https://www.ponylang.io/media/papers/a_prinicipled_design_of_capabilities_in_pony.pdf.
// * https://www.ponylang.io/media/papers/orca_gc_and_type_system_co-design_for_actor_languages.pdf.
// * https://www.ponylang.io/media/papers/formalizing-generics-for-pony.pdf.
//
// end of life.

use zo_ast::ast::Ast;
use zo_reporter::Result;
use zo_session::session::Session;

/// The representation of an analyzer.
struct Analyzer;
impl Analyzer {
  /// Analyses the AST and performs a bunch of analysis related to the semantic.
  fn analyze(&mut self, _session: &mut Session, ast: &Ast) -> Result<Ast> {
    Ok(ast.to_owned())
  }
}

/// Analyses the AST and performs a bunch of analysis related to the semantic.
pub fn analyze(session: &mut Session, ast: &Ast) -> Result<Ast> {
  Analyzer.analyze(session, ast)
}
