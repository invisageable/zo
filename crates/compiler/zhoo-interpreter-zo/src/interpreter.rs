use zhoo_ast::ast;
use zhoo_session::session::Session;
use zhoo_value::value::{Value, ValueKind};

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::Result;

struct Interpreter<'hir> {
  interner: &'hir mut Interner,
}

impl<'hir> Interpreter<'hir> {
  #[inline]
  fn new(interner: &'hir mut Interner) -> Self {
    Self { interner }
  }

  fn interpret(&mut self, program: &ast::Program) -> Result<Value> {
    let mut value = Value::unit();

    for item in &program.items {
      value = self.interpret_item(item)?;
    }

    Ok(value)
  }

  fn interpret_item(&mut self, item: &ast::Item) -> Result<Value> {
    match &item.kind {
      ast::ItemKind::Load(load) => self.interpret_item_load(load),
      ast::ItemKind::Pack(pack) => self.interpret_item_pack(pack),
      ast::ItemKind::Var(var) => self.interpret_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => {
        self.interpret_item_ty_alias(ty_alias)
      }
      ast::ItemKind::Ext(ext) => self.interpret_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.interpret_item_abstract(abstr),
      ast::ItemKind::Enum(enumeration) => self.interpret_item_enum(enumeration),
      ast::ItemKind::Struct(struct_decl) => {
        self.interpret_item_struct_decl(struct_decl)
      }
      ast::ItemKind::Apply(apply) => self.interpret_item_apply(apply),
      ast::ItemKind::Fun(fun) => self.interpret_item_fun(fun),
    }
  }

  fn interpret_item_load(&mut self, _var: &ast::Load) -> Result<Value> {
    todo!()
  }

  fn interpret_item_pack(&mut self, _var: &ast::Pack) -> Result<Value> {
    todo!()
  }

  fn interpret_item_ty_alias(
    &mut self,
    _ty_alias: &ast::TyAlias,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_item_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_var(var)
  }

  fn interpret_item_ext(&mut self, _ext: &ast::Ext) -> Result<Value> {
    todo!()
  }

  fn interpret_item_abstract(
    &mut self,
    _abstr: &ast::Abstract,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_item_enum(&mut self, _enumeration: &ast::Enum) -> Result<Value> {
    todo!()
  }

  fn interpret_item_struct_decl(
    &mut self,
    _struct_decl: &ast::Struct,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_item_apply(&mut self, _apply: &ast::Apply) -> Result<Value> {
    todo!()
  }

  fn interpret_item_fun(&mut self, fun: &ast::Fun) -> Result<Value> {
    let _prototype = self.interpret_prototype(&fun.prototype)?;
    let _body = self.interpret_block(&fun.body)?;

    Ok(Value::unit())
  }

  fn interpret_prototype(
    &mut self,
    _prototype: &ast::Prototype,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_block(&mut self, body: &ast::Block) -> Result<Value> {
    let mut value = Value::unit();

    for stmt in &body.stmts {
      value = self.interpret_stmt(stmt)?;
    }

    Ok(value)
  }

  fn interpret_stmt(&mut self, stmt: &ast::Stmt) -> Result<Value> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.interpret_stmt_var(var),
      ast::StmtKind::Item(item) => self.interpret_stmt_item(item),
      ast::StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  fn interpret_stmt_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_var(var)
  }

  fn interpret_var(&mut self, _var: &ast::Var) -> Result<Value> {
    todo!()
  }

  fn interpret_stmt_item(&mut self, _item: &ast::Item) -> Result<Value> {
    todo!()
  }

  fn interpret_stmt_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  fn interpret_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ast::ExprKind::UnOp(unop, lhs) => self.interpret_expr_unop(unop, lhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs)
      }
      ast::ExprKind::Assign(lhs, rhs) => self.interpret_expr_assign(lhs, rhs),
      ast::ExprKind::AssignOp(binop, lhs, rhs) => {
        self.interpret_expr_assignop(binop, lhs, rhs)
      }
      ast::ExprKind::Array(elmts) => self.interpret_expr_array(elmts),
      ast::ExprKind::Tuple(elmts) => self.interpret_expr_tuple(elmts),
      ast::ExprKind::ArrayAccess(array, access) => {
        self.interpret_expr_array_access(array, access)
      }
      ast::ExprKind::TupleAccess(tuple, access) => {
        self.interpret_expr_tuple_access(tuple, access)
      }
      ast::ExprKind::Block(block) => self.interpret_expr_block(block),
      ast::ExprKind::Fn(protoype, block) => {
        self.interpret_expr_fn(protoype, block)
      }
      ast::ExprKind::Call(callee, args) => {
        self.interpret_expr_call(callee, args)
      }
      ast::ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.interpret_expr_if_else(condition, consequence, maybe_alternative)
      }
      ast::ExprKind::When(condition, consequence, alternative) => {
        self.interpret_expr_when(condition, consequence, alternative)
      }
      ast::ExprKind::Match(condition, arms) => {
        self.interpret_expr_match(condition, arms)
      }
      ast::ExprKind::Loop(body) => self.interpret_expr_loop(body),
      ast::ExprKind::While(condition, body) => {
        self.interpret_expr_while(condition, body)
      }
      ast::ExprKind::Return(maybe_expr) => {
        self.interpret_expr_return(maybe_expr)
      }
      ast::ExprKind::Break(maybe_expr) => self.interpret_expr_break(maybe_expr),
      ast::ExprKind::Continue => self.interpret_expr_continue(),
      ast::ExprKind::Var(var) => self.interpret_expr_var(var),
      ast::ExprKind::StructExpr(struct_expr) => {
        self.interpret_expr_struct_expr(struct_expr)
      }
      ast::ExprKind::Chaining(lhs, rhs) => {
        self.interpret_expr_chaining(lhs, rhs)
      }
      _ => todo!(),
    }
  }

  fn interpret_expr_lit(&mut self, lit: &ast::Lit) -> Result<Value> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.interpret_expr_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.interpret_expr_lit_float(symbol),
      ast::LitKind::Ident(symbol) => self.interpret_expr_lit_ident(symbol),
      ast::LitKind::Bool(boolean) => self.interpret_expr_lit_bool(boolean),
      ast::LitKind::Char(symbol) => self.interpret_expr_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.interpret_expr_lit_str(symbol),
    }
  }

  fn interpret_expr_lit_int(&mut self, symbol: &Symbol) -> Result<Value> {
    let int = self.interner.lookup_int(*symbol);

    Ok(Value::int(int))
  }

  fn interpret_expr_lit_float(&mut self, symbol: &Symbol) -> Result<Value> {
    let float = self.interner.lookup_float(*symbol);

    Ok(Value::float(float))
  }

  fn interpret_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<Value> {
    let ident = self.interner.lookup_ident(*symbol);

    Ok(Value::ident(ident.into()))
  }

  fn interpret_expr_lit_bool(&mut self, boolean: &bool) -> Result<Value> {
    Ok(Value::bool(*boolean))
  }

  fn interpret_expr_lit_char(&mut self, symbol: &Symbol) -> Result<Value> {
    let char = self.interner.lookup_char(*symbol);

    Ok(Value::char(char))
  }

  fn interpret_expr_lit_str(&mut self, symbol: &Symbol) -> Result<Value> {
    let string = self.interner.lookup_str(*symbol);

    Ok(Value::str(string.into()))
  }

  fn interpret_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    lhs: &ast::Expr,
  ) -> Result<Value> {
    let value = self.interpret_expr(lhs)?;

    match &unop.kind {
      ast::UnOpKind::Neg => match &value.kind {
        ValueKind::Int(int) => Ok(Value::int(*int)),
        ValueKind::Float(float) => Ok(Value::float(*float)),
        _ => todo!(),
      },
      _ => todo!(),
    }
  }

  fn interpret_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
  ) -> Result<Value> {
    let lhs = self.interpret_expr(lhs)?;
    let rhs = self.interpret_expr(rhs)?;

    match (lhs.kind, rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        self.interpret_expr_binop_int(binop, lhs, rhs)
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        self.interpret_expr_binop_float(binop, lhs, rhs)
      }
      _ => todo!(),
    }
  }

  fn interpret_expr_binop_int(
    &mut self,
    binop: &ast::BinOp,
    lhs: i64,
    rhs: i64,
  ) -> Result<Value> {
    Ok(match &binop.kind {
      ast::BinOpKind::Add => Value::int(lhs + rhs),
      ast::BinOpKind::Sub => Value::int(lhs - rhs),
      ast::BinOpKind::Mul => Value::int(lhs * rhs),
      ast::BinOpKind::Div => Value::int(lhs / rhs),
      ast::BinOpKind::Rem => Value::int(lhs % rhs),
      _ => todo!(),
    })
  }

  fn interpret_expr_binop_float(
    &mut self,
    binop: &ast::BinOp,
    lhs: f64,
    rhs: f64,
  ) -> Result<Value> {
    Ok(match &binop.kind {
      ast::BinOpKind::Add => Value::float(lhs + rhs),
      ast::BinOpKind::Sub => Value::float(lhs - rhs),
      ast::BinOpKind::Mul => Value::float(lhs * rhs),
      ast::BinOpKind::Div => Value::float(lhs / rhs),
      ast::BinOpKind::Rem => Value::float(lhs % rhs),
      _ => todo!(),
    })
  }

  fn interpret_expr_assign(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_assignop(
    &mut self,
    _binop: &ast::BinOp,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_array(&mut self, _elements: &[ast::Expr]) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_tuple(&mut self, _elements: &[ast::Expr]) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_array_access(
    &mut self,
    _array: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_tuple_access(
    &mut self,
    _tuple: &ast::Expr,
    _access: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_block(&mut self, block: &ast::Block) -> Result<Value> {
    self.interpret_block(block)
  }

  fn interpret_expr_fn(
    &mut self,
    _prototype: &ast::Prototype,
    _block: &ast::Block,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_call(
    &mut self,
    _callee: &ast::Expr,
    _args: &ast::Args,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_if_else(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Block,
    _maybe_alternative: &Option<Box<ast::Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_when(
    &mut self,
    _condition: &ast::Expr,
    _consequence: &ast::Expr,
    _maybe_alternative: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_match(
    &mut self,
    _condition: &ast::Expr,
    _arms: &[ast::Arm],
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_loop(&mut self, _body: &ast::Block) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_while(
    &mut self,
    _condition: &ast::Expr,
    _body: &ast::Block,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_continue(&mut self) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_var(var)
  }

  fn interpret_expr_struct_expr(
    &mut self,
    _struct_expr: &ast::StructExpr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_chaining(
    &mut self,
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }
}

/// ...
///
/// ## examples.
///
/// ```
/// ```
pub fn interpret(
  session: &mut Session,
  program: &ast::Program,
) -> Result<Value> {
  Interpreter::new(&mut session.interner).interpret(program)
}
