use zhoo_ast::ast::Program;
use zhoo_session::session::Session;
use zhoo_tokenizer::token::Token;

use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::Result;

#[derive(Debug)]
struct Parser<'tokens> {
  #[allow(dead_code)]
  interner: &'tokens Interner,
  reporter: &'tokens Reporter,
  #[allow(dead_code)]
  tokens: &'tokens [Token],
}

impl<'tokens> Parser<'tokens> {
  #[inline]
  fn new(
    interner: &'tokens Interner,
    reporter: &'tokens Reporter,
    tokens: &'tokens [Token],
  ) -> Self {
    Self {
      interner,
      reporter,
      tokens,
    }
  }

  fn parse(&mut self) -> Result<Program> {
    let mut program = Program::new();

    let item = zhoo_ast::ast::Item {
      kind: zhoo_ast::ast::ItemKind::Fun(zhoo_ast::ast::Fun {
        body: zhoo_ast::ast::Block {
          stmts: vec![zhoo_ast::ast::Stmt {
            kind: zhoo_ast::ast::StmtKind::Expr(zhoo_ast::ast::Expr {
              kind: zhoo_ast::ast::ExprKind::Ident(String::from(
                "ijdoiejdoiej",
              )),
            }),
          }],
        },
      }),
    };

    program.add_item(item);

    self.reporter.abort_if_has_errors();

    Ok(program)
  }
}

pub fn parse(session: &mut Session, tokens: &[Token]) -> Result<Program> {
  println!("parse.");
  Parser::new(&session.interner, &session.reporter, tokens).parse()
}
