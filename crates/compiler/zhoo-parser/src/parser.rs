use zhoo_ast::ast::Program;

use zo_core::Result;

#[derive(Debug)]
pub struct Parser {}

impl Parser {
  #[inline]
  fn new() -> Self {
    Self {}
  }

  #[inline]
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

    Ok(program)
  }
}

pub fn parse() -> Result<Program> {
  println!("parse.");
  Parser::new().parse()
}
