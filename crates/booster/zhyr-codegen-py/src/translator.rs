use zhyr_ast::ast::{Ast, File, FileKind, ItemKind, Module, ModuleKind};

use zo_core::writer::Writer;
use zo_core::Result;

pub(crate) struct Translator {
  writer: Writer,
}

impl Translator {
  #[inline]
  pub fn new() -> Self {
    Self {
      writer: Writer::new(2usize),
    }
  }

  #[inline]
  pub fn output(&mut self) -> Result<Box<[u8]>> {
    Ok(self.writer.as_bytes())
  }

  pub fn translate(&mut self, ast: &Ast) -> Result<()> {
    for item in &ast.items {
      if let ItemKind::File(file) = &item.kind {
        self.translate_item_file(file)?;
      }
    }

    Ok(())
  }

  fn translate_item_file(&mut self, file: &File) -> Result<()> {
    match &file.kind {
      FileKind::Module(module) => self.translate_item_file_module(module),
      _ => todo!(),
    }
  }

  fn translate_item_file_module(&mut self, module: &Module) -> Result<()> {
    match &module.kind {
      ModuleKind::Py(module) => self.translate_mod(module),
      _ => unreachable!(),
    }
  }

  fn translate_mod(&mut self, module: &rustpython_ast::Mod) -> Result<()> {
    match module {
      rustpython_ast::Mod::Module(module) => self.translate_mod_module(module),
      rustpython_ast::Mod::Interactive(module) => {
        self.translate_mod_interactive(module)
      }
      rustpython_ast::Mod::Expression(module) => {
        self.translate_mod_expression(module)
      }
      rustpython_ast::Mod::FunctionType(module) => {
        self.translate_mod_function_type(module)
      }
    }
  }

  fn translate_mod_module(
    &mut self,
    module: &rustpython_ast::ModModule,
  ) -> Result<()> {
    for stmt in &module.body {
      self.translate_stmt(stmt)?;
    }

    Ok(())
  }

  fn translate_mod_interactive(
    &mut self,
    _module: &rustpython_ast::ModInteractive,
  ) -> Result<()> {
    todo!()
  }

  fn translate_mod_expression(
    &mut self,
    _module: &rustpython_ast::ModExpression,
  ) -> Result<()> {
    todo!()
  }

  fn translate_mod_function_type(
    &mut self,
    _module: &rustpython_ast::ModFunctionType,
  ) -> Result<()> {
    todo!()
  }

  fn translate_stmt(&mut self, stmt: &rustpython_ast::Stmt) -> Result<()> {
    match stmt {
      rustpython_ast::Stmt::Expr(expr) => self.translate_stmt_expr(expr),
      _ => todo!(),
    }
  }

  fn translate_stmt_expr(
    &mut self,
    stmt_expr: &rustpython_ast::StmtExpr,
  ) -> Result<()> {
    self.translate_expr(&stmt_expr.value)
  }

  fn translate_expr(&mut self, expr: &rustpython_ast::Expr) -> Result<()> {
    match expr {
      rustpython_ast::Expr::Call(call) => self.translate_expr_call(call),
      _ => todo!(),
    }
  }

  fn translate_expr_call(
    &mut self,
    call: &rustpython_ast::ExprCall,
  ) -> Result<()> {
    self.translate_expr(&call.func)?;
    self.translate_args(&call.args)
  }

  fn translate_args(
    &mut self,
    exprs: &Vec<rustpython_ast::Expr>,
  ) -> Result<()> {
    self.writer.write_bytes(b"(")?;

    for expr in exprs {
      self.translate_expr(expr)?;
    }

    self.writer.write_bytes(b")")
  }
}
