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
      println!("\n\nSTMT: {:?}\n", stmt);
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
      rustpython_ast::Stmt::FunctionDef(function_def) => {
        self.translate_stmt_function_def(function_def)
      }
      rustpython_ast::Stmt::Return(ret) => self.translate_stmt_return(ret),
      rustpython_ast::Stmt::If(if_else) => self.translate_stmt_if_else(if_else),
      rustpython_ast::Stmt::Expr(expr) => self.translate_stmt_expr(expr),
      kind => todo!("{kind:?}"),
    }
  }

  fn translate_stmt_function_def(
    &mut self,
    function_def: &rustpython_ast::StmtFunctionDef,
  ) -> Result<()> {
    self.writer.write_bytes(b"fun")?;
    self.writer.space()?;
    self.translate_identifier(&function_def.name)?;
    self.translate_arguments(&function_def.args)?;
    self.writer.space()?;
    self.translate_body(&function_def.body)
  }

  fn translate_stmt_return(
    &mut self,
    ret: &rustpython_ast::StmtReturn,
  ) -> Result<()> {
    self.writer.write_bytes(b"return")?;
    self.writer.space()?;

    match &ret.value {
      Some(expr) => self.translate_expr(expr),
      None => Ok(()),
    }
  }

  fn translate_identifier(
    &mut self,
    identifier: &rustpython_ast::Identifier,
  ) -> Result<()> {
    self.writer.write(identifier)
  }

  fn translate_arguments(
    &mut self,
    args: &rustpython_ast::Arguments,
  ) -> Result<()> {
    self.writer.write_bytes(b"(")?;

    for arg_with_default in &args.args {
      self.translate_argument_with_default(arg_with_default)?;
    }

    self.writer.write_bytes(b")")
  }

  fn translate_argument_with_default(
    &mut self,
    arg: &rustpython_ast::ArgWithDefault,
  ) -> Result<()> {
    self.translate_arg(&arg.def)
  }

  fn translate_arg(&mut self, arg: &rustpython_ast::Arg) -> Result<()> {
    self.translate_identifier(&arg.arg)
  }

  fn translate_body(
    &mut self,
    stmts: &Vec<rustpython_ast::Stmt>,
  ) -> Result<()> {
    self.writer.write_bytes(b"{")?;

    for stmt in stmts {
      self.translate_stmt(stmt)?;
    }

    self.writer.write_bytes(b"}")
  }

  fn translate_stmt_if_else(
    &mut self,
    if_else: &rustpython_ast::StmtIf,
  ) -> Result<()> {
    self.writer.write_bytes(b"if")?;
    self.writer.space()?;
    self.translate_expr(&if_else.test)?;
    self.translate_body(&if_else.body)?;
    self.writer.write_bytes(b"else")?;
    self.writer.space()?;
    self.translate_body(&if_else.orelse)
  }

  fn translate_stmt_expr(
    &mut self,
    stmt_expr: &rustpython_ast::StmtExpr,
  ) -> Result<()> {
    self.translate_expr(&stmt_expr.value)
  }

  fn translate_expr(&mut self, expr: &rustpython_ast::Expr) -> Result<()> {
    match expr {
      rustpython_ast::Expr::BinOp(binop) => self.translate_expr_binop(binop),
      rustpython_ast::Expr::Compare(compare) => {
        self.translate_expr_compare(compare)
      }
      rustpython_ast::Expr::Call(call) => self.translate_expr_call(call),
      rustpython_ast::Expr::Constant(constant) => {
        self.translate_expr_constant(constant)
      }
      rustpython_ast::Expr::Name(name) => self.translate_expr_name(name),
      kind => todo!("\n\n{kind:#?}\n"),
    }
  }

  fn translate_expr_binop(
    &mut self,
    binop: &rustpython_ast::ExprBinOp,
  ) -> Result<()> {
    self.translate_expr(&binop.left)?;
    self.writer.space()?;
    self.translate_op(&binop.op)?;
    self.writer.space()?;
    self.translate_expr(&binop.right)
  }

  fn translate_op(&mut self, op: &rustpython_ast::Operator) -> Result<()> {
    match op {
      rustpython_ast::Operator::Add => self.writer.write_bytes(b"+"),
      rustpython_ast::Operator::Sub => self.writer.write_bytes(b"-"),
      rustpython_ast::Operator::Mult => self.writer.write_bytes(b"*"),
      rustpython_ast::Operator::Div => self.writer.write_bytes(b"/"),
      rustpython_ast::Operator::Mod => self.writer.write_bytes(b"%"),
      rustpython_ast::Operator::LShift => self.writer.write_bytes(b"<<"),
      rustpython_ast::Operator::RShift => self.writer.write_bytes(b">>"),
      rustpython_ast::Operator::BitOr => todo!(),
      rustpython_ast::Operator::BitXor => todo!(),
      rustpython_ast::Operator::BitAnd => todo!(),
      rustpython_ast::Operator::FloorDiv => self.writer.write_bytes(b"/"),
      _ => todo!(),
    }
  }

  fn translate_expr_compare(
    &mut self,
    compare: &rustpython_ast::ExprCompare,
  ) -> Result<()> {
    self.translate_expr(&compare.left)?;
    self.writer.space()?;

    match &compare.ops[0] {
      rustpython_ast::CmpOp::Eq => self.writer.write_bytes(b"==")?,
      rustpython_ast::CmpOp::NotEq => self.writer.write_bytes(b"!=")?,
      rustpython_ast::CmpOp::Lt => self.writer.write_bytes(b"<")?,
      rustpython_ast::CmpOp::LtE => self.writer.write_bytes(b"<=")?,
      rustpython_ast::CmpOp::Gt => self.writer.write_bytes(b">")?,
      rustpython_ast::CmpOp::GtE => self.writer.write_bytes(b">=")?,
      _ => todo!(),
    };

    self.writer.space()?;

    self.translate_exprs(&compare.comparators)
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
      // todo — manage separator `,`
    }

    self.writer.write_bytes(b")")
  }

  fn translate_expr_constant(
    &mut self,
    constant: &rustpython_ast::ExprConstant,
  ) -> Result<()> {
    match &constant.value {
      rustpython_ast::Constant::None => Ok(()),
      rustpython_ast::Constant::Bool(boolean) => {
        self.translate_expr_constant_bool(boolean)
      }
      rustpython_ast::Constant::Str(string) => {
        self.translate_expr_constant_string(string)
      }
      rustpython_ast::Constant::Bytes(bytes) => {
        self.translate_expr_constant_bytes(bytes)
      }
      rustpython_ast::Constant::Int(big_int) => {
        self.translate_expr_constant_int(big_int)
      }
      rustpython_ast::Constant::Tuple(tuple) => {
        self.translate_expr_constant_tuple(tuple)
      }
      rustpython_ast::Constant::Float(float) => {
        self.translate_expr_constant_float(float)
      }
      rustpython_ast::Constant::Complex { real, imag } => {
        self.translate_expr_constant_complex(real, imag)
      }
      rustpython_ast::Constant::Ellipsis => {
        self.translate_expr_constant_ellipsis()
      }
    }
  }

  fn translate_expr_constant_bool(&mut self, boolean: &bool) -> Result<()> {
    self.writer.write(boolean)
  }

  fn translate_expr_constant_string(&mut self, string: &str) -> Result<()> {
    self.writer.write_bytes(b"\"")?;
    self.writer.write(string)?;
    self.writer.write_bytes(b"\"")
  }

  fn translate_expr_constant_bytes(&mut self, _bytes: &[u8]) -> Result<()> {
    todo!()
  }

  fn translate_expr_constant_int(
    &mut self,
    big_int: &rustpython_ast::bigint::BigInt,
  ) -> Result<()> {
    self.writer.write(big_int)
  }

  fn translate_expr_constant_tuple(
    &mut self,
    _tuple: &[rustpython_ast::Constant],
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_constant_float(&mut self, _float: &f64) -> Result<()> {
    todo!()
  }

  fn translate_expr_constant_complex(
    &mut self,
    _real: &f64,
    _imag: &f64,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_constant_ellipsis(&mut self) -> Result<()> {
    todo!()
  }

  fn translate_expr_name(
    &mut self,
    name: &rustpython_ast::ExprName,
  ) -> Result<()> {
    self.translate_identifier(&name.id)
  }

  fn translate_exprs(
    &mut self,
    exprs: &Vec<rustpython_ast::Expr>,
  ) -> Result<()> {
    for expr in exprs {
      self.translate_expr(expr)?;
    }

    Ok(())
  }
}
