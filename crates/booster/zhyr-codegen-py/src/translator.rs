//! ...

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
      writer: Writer::new(0usize),
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
      rustpython_ast::Stmt::FunctionDef(function_def) => {
        self.translate_stmt_function_def(function_def)
      }
      rustpython_ast::Stmt::AsyncFunctionDef(_asymc_function_def) => {
        todo!()
      }
      rustpython_ast::Stmt::ClassDef(_class_def) => {
        todo!()
      }
      rustpython_ast::Stmt::Return(ret) => self.translate_stmt_return(ret),
      rustpython_ast::Stmt::Delete(_delete) => {
        todo!()
      }
      rustpython_ast::Stmt::Assign(_assign) => {
        todo!()
      }
      rustpython_ast::Stmt::TypeAlias(_type_alias) => {
        todo!()
      }
      rustpython_ast::Stmt::AugAssign(_aug_assign) => {
        todo!()
      }
      rustpython_ast::Stmt::AnnAssign(_ann_assign) => {
        todo!()
      }
      rustpython_ast::Stmt::For(_for) => {
        todo!()
      }
      rustpython_ast::Stmt::AsyncFor(_async_for) => {
        todo!()
      }
      rustpython_ast::Stmt::While(_while) => {
        todo!()
      }
      rustpython_ast::Stmt::If(if_else) => self.translate_stmt_if_else(if_else),
      rustpython_ast::Stmt::With(_with) => {
        todo!()
      }
      rustpython_ast::Stmt::AsyncWith(_async_with) => {
        todo!()
      }
      rustpython_ast::Stmt::Match(_matching) => {
        todo!()
      }
      rustpython_ast::Stmt::Raise(_raise) => {
        todo!()
      }
      rustpython_ast::Stmt::Try(_trying) => {
        todo!()
      }
      rustpython_ast::Stmt::TryStar(_try_start) => {
        todo!()
      }
      rustpython_ast::Stmt::Assert(_assert) => {
        todo!()
      }
      rustpython_ast::Stmt::Import(_import) => {
        todo!()
      }
      rustpython_ast::Stmt::ImportFrom(_import_from) => {
        todo!()
      }
      rustpython_ast::Stmt::Global(_global) => {
        todo!()
      }
      rustpython_ast::Stmt::Nonlocal(_non_local) => {
        todo!()
      }
      rustpython_ast::Stmt::Expr(expr) => self.translate_stmt_expr(expr),
      rustpython_ast::Stmt::Pass(_pass) => {
        todo!()
      }
      rustpython_ast::Stmt::Break(_breaking) => {
        todo!()
      }
      rustpython_ast::Stmt::Continue(_continue) => {
        todo!()
      }
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
    }?;

    self.writer.semicolon()
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

    for (idx, arg_with_default) in args.args.iter().enumerate() {
      self.translate_argument_with_default(arg_with_default)?;

      if idx < args.args.len() - 1 {
        self.writer.comma()?;
        self.writer.space()?;
      }
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
    self.writer.space()?;
    self.writer.write_bytes(b"{")?;
    self.writer.new_line()?;

    for (idx, stmt) in stmts.iter().enumerate() {
      self.translate_stmt(stmt)?;

      if idx < stmts.len() - 1 {
        self.writer.new_line()?;
      }
    }

    self.writer.write_bytes(b"}")?;
    self.writer.new_line()
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
      rustpython_ast::Expr::BoolOp(bool_op) => {
        self.translate_expr_bool_op(bool_op)
      }
      rustpython_ast::Expr::NamedExpr(named) => {
        self.translate_expr_named(named)
      }
      rustpython_ast::Expr::BinOp(binop) => self.translate_expr_binop(binop),
      rustpython_ast::Expr::UnaryOp(unop) => self.translate_expr_unop(unop),
      rustpython_ast::Expr::Lambda(lambda) => {
        self.translate_expr_lambda(lambda)
      }
      rustpython_ast::Expr::IfExp(if_else) => {
        self.translate_expr_if_else(if_else)
      }
      rustpython_ast::Expr::Dict(dict) => self.translate_expr_dict(dict),
      rustpython_ast::Expr::Set(set) => self.translate_expr_set(set),
      rustpython_ast::Expr::ListComp(list_comp) => {
        self.translate_expr_list_comp(list_comp)
      }
      rustpython_ast::Expr::SetComp(set_comp) => {
        self.translate_expr_set_comp(set_comp)
      }
      rustpython_ast::Expr::DictComp(dict_comp) => {
        self.translate_expr_dict_comp(dict_comp)
      }
      rustpython_ast::Expr::GeneratorExp(generator) => {
        self.translate_expr_generator(generator)
      }
      rustpython_ast::Expr::Await(awaiting) => {
        self.translate_expr_await(awaiting)
      }
      rustpython_ast::Expr::Yield(yielding) => {
        self.translate_expr_yield(yielding)
      }
      rustpython_ast::Expr::YieldFrom(yield_from) => {
        self.translate_expr_yield_from(yield_from)
      }
      rustpython_ast::Expr::Compare(compare) => {
        self.translate_expr_compare(compare)
      }
      rustpython_ast::Expr::Call(call) => self.translate_expr_call(call),
      rustpython_ast::Expr::FormattedValue(formatted_value) => {
        self.translate_expr_formatted_value(formatted_value)
      }
      rustpython_ast::Expr::JoinedStr(joined_str) => {
        self.translate_expr_joined_str(joined_str)
      }
      rustpython_ast::Expr::Constant(constant) => {
        self.translate_expr_constant(constant)
      }
      rustpython_ast::Expr::Attribute(attribute) => {
        self.translate_expr_attribute(attribute)
      }
      rustpython_ast::Expr::Subscript(subscript) => {
        self.translate_expr_subscript(subscript)
      }
      rustpython_ast::Expr::Starred(starred) => {
        self.translate_expr_starred(starred)
      }
      rustpython_ast::Expr::Name(name) => self.translate_expr_name(name),
      rustpython_ast::Expr::List(list) => self.translate_expr_list(list),
      rustpython_ast::Expr::Tuple(tuple) => self.translate_expr_tuple(tuple),
      rustpython_ast::Expr::Slice(slice) => self.translate_expr_slice(slice),
    }
  }

  fn translate_expr_bool_op(
    &mut self,
    _bool_op: &rustpython_ast::ExprBoolOp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_named(
    &mut self,
    _named: &rustpython_ast::ExprNamedExpr,
  ) -> Result<()> {
    todo!()
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

  fn translate_expr_unop(
    &mut self,
    _unop: &rustpython_ast::ExprUnaryOp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_lambda(
    &mut self,
    _lambda: &rustpython_ast::ExprLambda,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_if_else(
    &mut self,
    _if_else: &rustpython_ast::ExprIfExp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_dict(
    &mut self,
    _dict: &rustpython_ast::ExprDict,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_set(
    &mut self,
    _set: &rustpython_ast::ExprSet,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_list_comp(
    &mut self,
    _list_cpom: &rustpython_ast::ExprListComp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_set_comp(
    &mut self,
    _set_comp: &rustpython_ast::ExprSetComp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_dict_comp(
    &mut self,
    _dict_comp: &rustpython_ast::ExprDictComp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_generator(
    &mut self,
    _generator: &rustpython_ast::ExprGeneratorExp,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_await(
    &mut self,
    _awaiting: &rustpython_ast::ExprAwait,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_yield(
    &mut self,
    _yielding: &rustpython_ast::ExprYield,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_yield_from(
    &mut self,
    _yield_from: &rustpython_ast::ExprYieldFrom,
  ) -> Result<()> {
    todo!()
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

  fn translate_expr_formatted_value(
    &mut self,
    _formatted_value: &rustpython_ast::ExprFormattedValue,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_joined_str(
    &mut self,
    _joined_str: &rustpython_ast::ExprJoinedStr,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_attribute(
    &mut self,
    _attribute: &rustpython_ast::ExprAttribute,
  ) -> Result<()> {
    todo!()
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

  fn translate_expr_subscript(
    &mut self,
    _subscript: &rustpython_ast::ExprSubscript,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_starred(
    &mut self,
    _starred: &rustpython_ast::ExprStarred,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_name(
    &mut self,
    name: &rustpython_ast::ExprName,
  ) -> Result<()> {
    self.translate_identifier(&name.id)
  }

  fn translate_expr_list(
    &mut self,
    _list: &rustpython_ast::ExprList,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_tuple(
    &mut self,
    _tuple: &rustpython_ast::ExprTuple,
  ) -> Result<()> {
    todo!()
  }

  fn translate_expr_slice(
    &mut self,
    _slice: &rustpython_ast::ExprSlice,
  ) -> Result<()> {
    todo!()
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
