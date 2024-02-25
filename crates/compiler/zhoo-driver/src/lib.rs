pub fn main() {
  let program = zhoo_ast::ast::Program {
    items: vec![zhoo_ast::ast::Item {
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
    }],
  };

  let ty = zhoo_infer::infer::infer(&program).unwrap();

  println!("INFER: {ty:?}");
}
