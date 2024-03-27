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
      ModuleKind::Js(module) => self.translate_module(module),
      _ => unreachable!(),
    }
  }

  fn translate_module(&mut self, module: &swc_ecma_ast::Module) -> Result<()> {
    for module_item in &module.body {
      self.translate_module_item(module_item)?;
    }

    Ok(())
  }

  fn translate_module_item(
    &mut self,
    module_item: &swc_ecma_ast::ModuleItem,
  ) -> Result<()> {
    match module_item {
      swc_ecma_ast::ModuleItem::ModuleDecl(module_decl) => {
        self.translate_module_decl(module_decl)
      }
      swc_ecma_ast::ModuleItem::Stmt(stmt) => self.translate_stmt(stmt),
    }
  }

  fn translate_module_decl(
    &mut self,
    module_decl: &swc_ecma_ast::ModuleDecl,
  ) -> Result<()> {
    match module_decl {
      swc_ecma_ast::ModuleDecl::Import(import_decl) => {
        self.translate_module_decl_import(import_decl)
      }
      _ => Ok(()),
    }
  }

  fn translate_module_decl_import(
    &mut self,
    _import_decl: &swc_ecma_ast::ImportDecl,
  ) -> Result<()> {
    Ok(())
  }

  fn translate_stmt(&mut self, stmt: &swc_ecma_ast::Stmt) -> Result<()> {
    match stmt {
      swc_ecma_ast::Stmt::Decl(decl) => self.translate_stmt_decl(decl),
      swc_ecma_ast::Stmt::Expr(expr) => self.translate_stmt_expr(expr),
      _ => Ok(()),
    }
  }

  fn translate_stmt_decl(&mut self, stmt: &swc_ecma_ast::Decl) -> Result<()> {
    match stmt {
      swc_ecma_ast::Decl::Fn(decl_fn) => self.translate_stmt_decl_fn(decl_fn),
      _ => Ok(()),
    }
  }

  fn translate_stmt_decl_fn(
    &mut self,
    _fn_decl: &swc_ecma_ast::FnDecl,
  ) -> Result<()> {
    todo!()
  }

  fn translate_stmt_expr(
    &mut self,
    expr_stmt: &swc_ecma_ast::ExprStmt,
  ) -> Result<()> {
    self.translate_expr(expr_stmt.expr.as_ref())?;
    self.writer.semicolon()
  }

  fn translate_expr(&mut self, expr: &swc_ecma_ast::Expr) -> Result<()> {
    match expr {
      swc_ecma_ast::Expr::Call(call) => self.translate_expr_call(call),
      swc_ecma_ast::Expr::Ident(ident) => self.translate_expr_ident(ident),
      swc_ecma_ast::Expr::Member(member) => self.translate_expr_member(member),
      swc_ecma_ast::Expr::Lit(lit) => self.translate_expr_lit(lit),
      _ => panic!(),
    }
  }

  fn translate_expr_call(
    &mut self,
    call: &swc_ecma_ast::CallExpr,
  ) -> Result<()> {
    // println!("\nCALL.\n");

    // if let swc_ecma_ast::Callee::Expr(expr) = &call.callee {
    //   println!("\nCALL EXPR: {expr:?}\n");
    //   if let swc_ecma_ast::Expr::Ident(ident) = expr.as_ref() {
    //     // if ident.sym == "consol"
    //     println!("{ident}");
    //   }
    // }

    self.translate_callee(&call.callee)?;
    self.translate_args(&call.args)
  }

  fn translate_callee(&mut self, callee: &swc_ecma_ast::Callee) -> Result<()> {
    match callee {
      swc_ecma_ast::Callee::Super(access) => self.translate_super(access),
      swc_ecma_ast::Callee::Import(import) => self.translate_import(import),
      swc_ecma_ast::Callee::Expr(expr) => self.translate_expr(expr),
    }
  }

  fn translate_super(&mut self, _access: &swc_ecma_ast::Super) -> Result<()> {
    self.writer.write_bytes(b"super")
  }

  fn translate_import(&mut self, _import: &swc_ecma_ast::Import) -> Result<()> {
    todo!()
  }

  fn translate_args(
    &mut self,
    args: &Vec<swc_ecma_ast::ExprOrSpread>,
  ) -> Result<()> {
    self.writer.write_bytes(b"(")?;

    for expr in args {
      self.translate_expr_or_spread(expr)?;
    }

    self.writer.write_bytes(b")")
  }

  fn translate_expr_or_spread(
    &mut self,
    expr_or_spread: &swc_ecma_ast::ExprOrSpread,
  ) -> Result<()> {
    match &expr_or_spread.spread {
      Some(_span) => todo!(),
      None => self.translate_expr(&expr_or_spread.expr),
    }
  }

  fn translate_expr_ident(
    &mut self,
    ident: &swc_ecma_ast::Ident,
  ) -> Result<()> {
    self.writer.write(&ident.sym)
  }

  fn translate_expr_member(
    &mut self,
    member: &swc_ecma_ast::MemberExpr,
  ) -> Result<()> {
    if let swc_ecma_ast::Expr::Ident(ident) = member.obj.as_ref() {
      if ident.sym == "console" {
        if let swc_ecma_ast::MemberProp::Ident(ident) = &member.prop {
          if ident.sym == "log" {
            return self.writer.write_bytes(b"println");
          }
        }
      }
    }

    self.translate_expr(&member.obj)?;
    self.writer.period()?;
    self.translate_member_prop(&member.prop)
  }

  fn translate_member_prop(
    &mut self,
    member_prop: &swc_ecma_ast::MemberProp,
  ) -> Result<()> {
    match member_prop {
      swc_ecma_ast::MemberProp::Ident(ident) => {
        self.translate_expr_ident(ident)
      }
      swc_ecma_ast::MemberProp::PrivateName(private_name) => {
        self.translate_private_name(private_name)
      }
      swc_ecma_ast::MemberProp::Computed(computed_prop_name) => {
        self.translate_computed_prop_name(computed_prop_name)
      }
    }
  }

  fn translate_private_name(
    &mut self,
    private_name: &swc_ecma_ast::PrivateName,
  ) -> Result<()> {
    self.translate_expr_ident(&private_name.id)
  }

  fn translate_computed_prop_name(
    &mut self,
    computed_prop_name: &swc_ecma_ast::ComputedPropName,
  ) -> Result<()> {
    self.translate_expr(&computed_prop_name.expr)
  }

  fn translate_expr_lit(&mut self, lit: &swc_ecma_ast::Lit) -> Result<()> {
    match lit {
      swc_ecma_ast::Lit::Str(string) => self.translate_expr_lit_str(string),
      swc_ecma_ast::Lit::Bool(boolean) => self.translate_expr_lit_bool(boolean),
      swc_ecma_ast::Lit::Null(null) => self.translate_expr_lit_null(null),
      swc_ecma_ast::Lit::Num(number) => self.translate_expr_lit_number(number),
      swc_ecma_ast::Lit::BigInt(big_int) => {
        self.translate_expr_lit_big_int(big_int)
      }
      swc_ecma_ast::Lit::Regex(regex) => self.translate_expr_lit_regex(regex),
      swc_ecma_ast::Lit::JSXText(jsx_text) => {
        self.translate_expr_lit_jsx_text(jsx_text)
      }
    }
  }

  fn translate_expr_lit_str(
    &mut self,
    string: &swc_ecma_ast::Str,
  ) -> Result<()> {
    self.writer.write_bytes(b"\"")?;
    self.writer.write(&string.value)?;
    self.writer.write_bytes(b"\"")
  }

  fn translate_expr_lit_bool(
    &mut self,
    boolean: &swc_ecma_ast::Bool,
  ) -> Result<()> {
    self.writer.write(&boolean.value)
  }

  fn translate_expr_lit_null(
    &mut self,
    _null: &swc_ecma_ast::Null,
  ) -> Result<()> {
    self.writer.write_bytes(b"null")
  }

  fn translate_expr_lit_number(
    &mut self,
    number: &swc_ecma_ast::Number,
  ) -> Result<()> {
    self.writer.write(&number.value)
  }

  fn translate_expr_lit_big_int(
    &mut self,
    big_int: &swc_ecma_ast::BigInt,
  ) -> Result<()> {
    self.writer.write(&big_int.value)
  }

  fn translate_expr_lit_regex(
    &mut self,
    _regex: &swc_ecma_ast::Regex,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_lit_jsx_text(
    &mut self,
    _jsx_text: &swc_ecma_ast::JSXText,
  ) -> Result<()> {
    todo!()
  }
}
