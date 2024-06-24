//! ...

// todo #1: should returns reporter error and call `?` operator instead of
// unwrap method.

use super::scope::ScopeMap;

use zo_ast::ast::{
  Arg, Args, Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Ext, Fun, Item,
  ItemKind, Lit, LitKind, Prototype, Stmt, StmtKind, Struct, StructExpr,
  TyAlias, UnOp, UnOpKind, Var,
};

use zo_value::builtin::BuiltinFn;
use zo_value::value;
use zo_value::value::RecordKey;
use zo_value::value::{Array, StructExprKey, Value, ValueKind};

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::interner::Interner;
use zo_core::reporter::report::eval::Eval;
use zo_core::reporter::report::ReportError;
use zo_core::reporter::Reporter;
use zo_core::span::{AsSpan, Span};
use zo_core::Result;

use hashbrown::HashMap;
use smol_str::{SmolStr, ToSmolStr};

#[derive(Debug)]
pub struct Interpreter<'ast> {
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
  scope_map: ScopeMap,
}

impl<'ast> Interpreter<'ast> {
  #[inline]
  pub fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      scope_map: ScopeMap::new(),
    }
  }

  fn interpret_block(&mut self, block: &Block) -> Result<Value> {
    let mut value = Value::UNIT;

    for stmt in block.iter() {
      value = self.interpret_stmt(stmt)?;

      if let ValueKind::Return(value) = value.kind {
        return Ok(*value);
      }
    }

    Ok(value)
  }

  pub fn interpret(&mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    self.scope_map.scope_entry();

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(report_error) => self.reporter.raise(report_error),
      };

      if let ValueKind::Return(value) = value.kind {
        return Ok(*value);
      }
    }

    self.scope_map.scope_exit();

    Ok(value)
  }

  fn interpret_item(&mut self, item: &Item) -> Result<Value> {
    match &item.kind {
      ItemKind::Var(var) => self.interpret_item_var(var),
      ItemKind::TyAlias(ty_alias) => self.interpret_item_ty_alias(ty_alias),
      ItemKind::Ext(ext) => self.interpret_item_ext(ext),
      ItemKind::Struct(strctr) => self.interpret_item_struct(strctr),
      ItemKind::Fun(fun) => self.interpret_item_fun(fun),
    }
  }

  fn interpret_item_var(&mut self, var: &Var) -> Result<Value> {
    self.interpret_var(var)
  }

  fn interpret_item_ty_alias(&mut self, _ty_alias: &TyAlias) -> Result<Value> {
    todo!()
  }

  fn interpret_item_ext(&mut self, _ext: &Ext) -> Result<Value> {
    todo!()
  }

  fn interpret_item_struct(&mut self, strctr: &Struct) -> Result<Value> {
    let value = Value::strctr(strctr.ident, strctr.fields.clone(), strctr.span);

    self
      .scope_map
      .add_fun(strctr.ident.name, value.to_owned())
      .unwrap();

    Ok(value)
  }

  fn interpret_item_fun(&mut self, fun: &Fun) -> Result<Value> {
    let value =
      Value::fun(fun.prototype.to_owned(), fun.body.to_owned(), fun.span);

    self
      .scope_map
      .add_fun(*fun.prototype.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    match &stmt.kind {
      StmtKind::Var(var) => self.interpret_stmt_var(var),
      StmtKind::Item(item) => self.interpret_stmt_item(item),
      StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  fn interpret_stmt_var(&mut self, var: &Var) -> Result<Value> {
    self.interpret_var(var)
  }

  fn interpret_var(&mut self, var: &Var) -> Result<Value> {
    let value = self.interpret_expr(&var.value)?;

    self
      .scope_map
      .add_var(*var.pattern.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_stmt_item(&mut self, item: &Item) -> Result<Value> {
    self.interpret_item(item)
  }

  fn interpret_stmt_expr(&mut self, expr: &Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  fn interpret_expr(&mut self, expr: &Expr) -> Result<Value> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ExprKind::UnOp(unop, rhs) => {
        self.interpret_expr_unop(unop, rhs, expr.span)
      }
      ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs, expr.span)
      }
      ExprKind::Assign(assignee, value) => {
        self.interpret_expr_assign(assignee, value)
      }
      ExprKind::AssignOp(binop, assignee, value) => {
        self.interpret_expr_assign_op(binop, assignee, value, expr.span)
      }
      ExprKind::Block(block) => self.interpret_expr_block(block),
      ExprKind::Fn(prototype, block) => {
        self.interpret_expr_fn(prototype, block, expr.span)
      }
      ExprKind::Call(callee, args) => self.interpret_expr_call(callee, args),
      ExprKind::Array(elmts) => self.interpret_expr_array(elmts, expr.span),
      ExprKind::ArrayAccess(indexed, index) => {
        self.interpret_expr_array_access(indexed, index, expr.span)
      }
      ExprKind::Struct(structure) => {
        self.interpret_expr_struct(structure, expr.span)
      }
      ExprKind::StructAccess(structure, prop) => {
        self.interpret_expr_struct_access(structure, prop, expr.span)
      }
      ExprKind::Record(pairs) => self.interpret_expr_record(pairs, expr.span),
      ExprKind::RecordAccess(record, prop) => {
        self.interpret_expr_record_access(record, prop, expr.span)
      }
      ExprKind::IfElse(condition, consequence, maybe_alternative) => self
        .interpret_expr_if_else(
          condition,
          consequence,
          maybe_alternative,
          expr.span,
        ),
      ExprKind::When(condition, consequence, alternative) => {
        self.interpret_expr_when(condition, consequence, alternative)
      }
      ExprKind::Loop(body) => self.interpret_expr_loop(body),
      ExprKind::While(condition, body) => {
        self.interpret_expr_while(condition, body)
      }
      ExprKind::Return(maybe_expr) => {
        self.interpret_expr_return(maybe_expr, expr.span)
      }
      ExprKind::Break(maybe_expr) => {
        self.interpret_expr_break(maybe_expr, expr.span)
      }
      ExprKind::Continue => self.interpret_expr_continue(),
      ExprKind::Var(var) => self.interpret_expr_var(var),
    }
  }

  fn interpret_expr_lit(&mut self, lit: &Lit) -> Result<Value> {
    match &lit.kind {
      LitKind::Int(symbol) => self.interpret_expr_lit_int(symbol, lit.span),
      LitKind::Float(symbol) => self.interpret_expr_lit_float(symbol, lit.span),
      LitKind::Ident(symbol) => self.interpret_expr_lit_ident(symbol, lit.span),
      LitKind::Bool(boolean) => self.interpret_expr_lit_bool(boolean, lit.span),
      LitKind::Char(symbol) => self.interpret_expr_lit_char(symbol, lit.span),
      LitKind::Str(symbol) => self.interpret_expr_lit_str(symbol, lit.span),
    }
  }

  fn interpret_expr_lit_int(
    &mut self,
    symbol: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let int = self.interner.lookup_int(symbol);

    Ok(Value::int(int, span))
  }

  fn interpret_expr_lit_float(
    &mut self,
    symbol: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let float = self.interner.lookup_float(symbol);

    Ok(Value::float(float, span))
  }

  fn interpret_expr_lit_ident(
    &mut self,
    symbol: &Symbol,
    span: Span,
  ) -> Result<Value> {
    if let Some(var) = self.scope_map.var(symbol) {
      return Ok(var.to_owned());
    } else if let Some(fun) = self.scope_map.fun(symbol) {
      return Ok(fun.to_owned());
    }

    // it should be better to adds a scope for record because actually we are
    // not able to throw an error when an identifier is not recognized.
    Ok(Value::ident(*symbol, span))

    // let ident = self.interner.lookup_ident(symbol);

    // Err(ReportError::Eval(Eval::IdentNotFound(
    //   span,
    //   ident.to_string(),
    // )))
  }

  fn interpret_expr_lit_bool(
    &mut self,
    boolean: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*boolean, span))
  }

  fn interpret_expr_lit_char(
    &mut self,
    symbol: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let ch = self.interner.lookup_char(symbol);

    Ok(Value::char(ch, span))
  }

  fn interpret_expr_lit_str(
    &mut self,
    symbol: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let string = self.interner.lookup_str(symbol).replace("\"", "");

    Ok(Value::str(string.to_smolstr(), span))
  }

  fn interpret_expr_unop(
    &mut self,
    unop: &UnOp,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    let value = self.interpret_expr(rhs)?;

    match unop.kind {
      UnOpKind::Neg => self.interpret_expr_unop_neg(value, span),
      UnOpKind::Not => self.interpret_expr_unop_not(value, span),
    }
  }

  fn interpret_expr_unop_neg(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    match rhs.kind {
      ValueKind::Int(int) => Ok(Value::int(-int, span)),
      ValueKind::Float(float) => Ok(Value::float(-float, span)),
      _ => Err(ReportError::Eval(Eval::UnknownUnOp(span, rhs.to_string()))),
    }
  }

  fn interpret_expr_unop_not(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(!rhs.as_bool(), span))
  }

  fn interpret_expr_binop(
    &mut self,
    binop: &BinOp,
    lhs: &Expr,
    rhs: &Expr,
    span: Span,
  ) -> Result<Value> {
    let lhs = self.interpret_expr(lhs)?;
    let rhs = self.interpret_expr(rhs)?;

    match (&lhs.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        self.interpret_expr_binop_int(binop, lhs, rhs, span)
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        self.interpret_expr_binop_float(binop, lhs, rhs, span)
      }
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        self.interpret_expr_binop_bool(binop, lhs, rhs, span)
      }
      (ValueKind::Str(lhs), ValueKind::Str(rhs)) => {
        self.interpret_expr_binop_str(binop, lhs, rhs, span)
      }
      _ => Err(ReportError::Eval(Eval::UnknownBinOpOperand(
        span,
        lhs.to_string(),
        rhs.to_string(),
      ))),
    }
  }

  fn interpret_expr_binop_int(
    &mut self,
    binop: &BinOp,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => self.interpret_expr_binop_int_add(lhs, rhs, span),
      BinOpKind::Sub => self.interpret_expr_binop_int_sub(lhs, rhs, span),
      BinOpKind::Mul => self.interpret_expr_binop_int_mul(lhs, rhs, span),
      BinOpKind::Div => self.interpret_expr_binop_int_div(lhs, rhs, span),
      BinOpKind::Rem => self.interpret_expr_binop_int_rem(lhs, rhs, span),
      BinOpKind::Lt => self.interpret_expr_binop_int_lt(lhs, rhs, span),
      BinOpKind::Gt => self.interpret_expr_binop_int_gt(lhs, rhs, span),
      BinOpKind::Le => self.interpret_expr_binop_int_le(lhs, rhs, span),
      BinOpKind::Ge => self.interpret_expr_binop_int_ge(lhs, rhs, span),
      BinOpKind::Eq => self.interpret_expr_binop_int_eq(lhs, rhs, span),
      BinOpKind::Ne => self.interpret_expr_binop_int_ne(lhs, rhs, span),
      BinOpKind::Shl => self.interpret_expr_binop_int_shl(lhs, rhs, span),
      BinOpKind::Shr => self.interpret_expr_binop_int_shr(lhs, rhs, span),
      _ => Err(ReportError::Eval(Eval::UnknownBinOp(
        binop.span,
        binop.to_string(),
      ))),
    }
  }

  fn interpret_expr_binop_int_add(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs + rhs, span))
  }

  fn interpret_expr_binop_int_sub(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs - rhs, span))
  }

  fn interpret_expr_binop_int_mul(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs * rhs, span))
  }

  fn interpret_expr_binop_int_div(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs / rhs, span))
  }

  fn interpret_expr_binop_int_rem(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs % rhs, span))
  }

  fn interpret_expr_binop_int_lt(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs < rhs, span))
  }

  fn interpret_expr_binop_int_gt(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs > rhs, span))
  }

  fn interpret_expr_binop_int_le(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs <= rhs, span))
  }

  fn interpret_expr_binop_int_ge(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs >= rhs, span))
  }

  fn interpret_expr_binop_int_eq(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs == rhs, span))
  }

  fn interpret_expr_binop_int_ne(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(lhs != rhs, span))
  }

  fn interpret_expr_binop_int_shl(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs << rhs, span))
  }

  fn interpret_expr_binop_int_shr(
    &mut self,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::int(lhs >> rhs, span))
  }

  fn interpret_expr_binop_float(
    &mut self,
    binop: &BinOp,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => self.interpret_expr_binop_float_add(lhs, rhs, span),
      BinOpKind::Sub => self.interpret_expr_binop_float_sub(lhs, rhs, span),
      BinOpKind::Mul => self.interpret_expr_binop_float_mul(lhs, rhs, span),
      BinOpKind::Div => self.interpret_expr_binop_float_div(lhs, rhs, span),
      BinOpKind::Rem => self.interpret_expr_binop_float_rem(lhs, rhs, span),
      BinOpKind::Lt => self.interpret_expr_binop_float_lt(lhs, rhs, span),
      BinOpKind::Gt => self.interpret_expr_binop_float_gt(lhs, rhs, span),
      BinOpKind::Le => self.interpret_expr_binop_float_le(lhs, rhs, span),
      BinOpKind::Ge => self.interpret_expr_binop_float_ge(lhs, rhs, span),
      BinOpKind::Eq => self.interpret_expr_binop_float_eq(lhs, rhs, span),
      BinOpKind::Ne => self.interpret_expr_binop_float_ne(lhs, rhs, span),
      _ => Err(ReportError::Eval(Eval::UnknownBinOp(
        binop.span,
        binop.to_string(),
      ))),
    }
  }

  fn interpret_expr_binop_float_add(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::float(lhs + rhs, span))
  }

  fn interpret_expr_binop_float_sub(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::float(lhs - rhs, span))
  }

  fn interpret_expr_binop_float_mul(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::float(lhs * rhs, span))
  }

  fn interpret_expr_binop_float_div(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::float(lhs / rhs, span))
  }

  fn interpret_expr_binop_float_rem(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::float(lhs % rhs, span))
  }

  fn interpret_expr_binop_float_lt(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs < *rhs, span))
  }

  fn interpret_expr_binop_float_gt(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs > *rhs, span))
  }

  fn interpret_expr_binop_float_le(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs <= *rhs, span))
  }

  fn interpret_expr_binop_float_ge(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs >= *rhs, span))
  }

  fn interpret_expr_binop_float_eq(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs == *rhs, span))
  }

  fn interpret_expr_binop_float_ne(
    &mut self,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs != *rhs, span))
  }

  fn interpret_expr_binop_bool(
    &mut self,
    binop: &BinOp,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::And => self.interpret_expr_binop_bool_and(lhs, rhs, span),
      BinOpKind::Or => self.interpret_expr_binop_bool_or(lhs, rhs, span),
      BinOpKind::BitAnd => {
        self.interpret_expr_binop_bool_bit_and(lhs, rhs, span)
      }
      BinOpKind::BitOr => self.interpret_expr_binop_bool_bit_or(lhs, rhs, span),
      BinOpKind::BitXor => {
        self.interpret_expr_binop_bool_bit_xor(lhs, rhs, span)
      }
      _ => Err(ReportError::Eval(Eval::UnknownBinOp(
        binop.span,
        binop.to_string(),
      ))),
    }
  }

  fn interpret_expr_binop_bool_and(
    &mut self,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs && *rhs, span))
  }

  fn interpret_expr_binop_bool_or(
    &mut self,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs || *rhs, span))
  }

  fn interpret_expr_binop_bool_bit_and(
    &mut self,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs & *rhs, span))
  }

  fn interpret_expr_binop_bool_bit_or(
    &mut self,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs | *rhs, span))
  }

  fn interpret_expr_binop_bool_bit_xor(
    &mut self,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs ^ *rhs, span))
  }

  fn interpret_expr_binop_str(
    &mut self,
    binop: &BinOp,
    lhs: &SmolStr,
    rhs: &SmolStr,
    span: Span,
  ) -> Result<Value> {
    match binop.kind {
      BinOpKind::Add => self.interpret_expr_binop_str_and(lhs, rhs, span),
      _ => Err(ReportError::Eval(Eval::UnknownBinOp(
        binop.span,
        binop.to_string(),
      ))),
    }
  }

  fn interpret_expr_binop_str_and(
    &mut self,
    lhs: &SmolStr,
    rhs: &SmolStr,
    span: Span,
  ) -> Result<Value> {
    let mut string = String::with_capacity(lhs.len() + rhs.len());

    string.push_str(lhs);
    string.push_str(rhs);

    Ok(Value::str(string.to_smolstr(), span))
  }

  fn interpret_expr_assign(
    &mut self,
    assignee: &Expr,
    value: &Expr,
  ) -> Result<Value> {
    let value = self.interpret_expr(value)?;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op(
    &mut self,
    binop: &BinOp,
    assignee: &Expr,
    value: &Expr,
    _span: Span,
  ) -> Result<Value> {
    let lhs = match self.scope_map.var(assignee.as_symbol()) {
      Some(value) => value.to_owned(),
      None => panic!(), // returns reporter error.
    };

    let rhs = self.interpret_expr(value)?;

    match binop.kind {
      BinOpKind::Add => self.interpret_expr_assign_op_add(assignee, &lhs, &rhs),
      BinOpKind::Sub => self.interpret_expr_assign_op_sub(assignee, &lhs, &rhs),
      BinOpKind::Mul => self.interpret_expr_assign_op_mul(assignee, &lhs, &rhs),
      BinOpKind::Div => self.interpret_expr_assign_op_div(assignee, &lhs, &rhs),
      BinOpKind::Rem => self.interpret_expr_assign_op_rem(assignee, &lhs, &rhs),
      BinOpKind::BitAnd => {
        self.interpret_expr_assign_op_bit_and(assignee, &lhs, &rhs)
      }
      BinOpKind::BitOr => {
        self.interpret_expr_assign_op_bit_or(assignee, &lhs, &rhs)
      }
      BinOpKind::BitXor => {
        self.interpret_expr_assign_op_bit_xor(assignee, &lhs, &rhs)
      }
      BinOpKind::Shl => self.interpret_expr_assign_op_shl(assignee, &lhs, &rhs),
      BinOpKind::Shr => self.interpret_expr_assign_op_shr(assignee, &lhs, &rhs),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_assign_op_add(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs + rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_sub(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs - rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_mul(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs * rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_div(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs / rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_rem(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs % rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_bit_and(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs & rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_bit_or(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs | rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_bit_xor(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs | rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_shl(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs << rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_assign_op_shr(
    &mut self,
    assignee: &Expr,
    lhs: &Value,
    rhs: &Value,
  ) -> Result<Value> {
    let value = lhs >> rhs;

    self
      .scope_map
      .set_var(*assignee.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_block(&mut self, block: &Block) -> Result<Value> {
    self.interpret_block(block)
  }

  fn interpret_expr_fn(
    &mut self,
    prototype: &Prototype,
    block: &Block,
    span: Span,
  ) -> Result<Value> {
    let value = Value::closure(prototype.to_owned(), block.to_owned(), span);

    self
      .scope_map
      .add_fun(*prototype.as_symbol(), value.to_owned())
      .unwrap(); // todo #1.

    Ok(value)
  }

  fn interpret_expr_call(
    &mut self,
    callee: &Expr,
    args: &Args,
  ) -> Result<Value> {
    let callee = self.interpret_expr(callee)?;
    let args = self.interpret_args(args)?;

    match callee.kind {
      ValueKind::Fn(prototype, block) => {
        self.interpret_expr_call_fn(prototype, block, args)
      }
      ValueKind::Builtin(builtin) => {
        self.interpret_expr_call_builtin(builtin, args)
      }
      _ => Err(ReportError::Eval(Eval::UnknownCallee(
        callee.span,
        callee.to_string(),
      ))),
    }
  }

  fn interpret_args(&mut self, args: &Args) -> Result<value::Args> {
    let mut args_new = value::Args::new();

    if args.is_empty() {
      return Ok(args_new);
    }

    for arg in args.iter() {
      args_new.add_arg(self.interpret_arg(arg)?);
    }

    Ok(args_new)
  }

  fn interpret_arg(&mut self, arg: &Arg) -> Result<value::Arg> {
    let value = self.interpret_expr(&arg.expr)?;

    Ok(value::Arg {
      value,
      span: arg.span,
    })
  }

  fn interpret_expr_call_fn(
    &mut self,
    prototype: Prototype,
    block: Block,
    args: value::Args,
  ) -> Result<Value> {
    if prototype.inputs.len() != args.len() {
      return Err(ReportError::Eval(Eval::MismatchArgument(
        args.as_span(),
        prototype.inputs.len(),
        args.len(),
      )));
    }

    self.scope_map.scope_entry();

    for (idx, input) in prototype.inputs.iter().enumerate() {
      let arg = args.get(idx).unwrap();

      self
        .scope_map
        .add_var(*input.as_symbol(), arg.value.to_owned())
        .unwrap(); // todo #1.
    }

    let value = self.interpret_expr_block(&block)?;

    self.scope_map.scope_exit();

    match value.kind {
      ValueKind::Return(value) => Ok(*value),
      _ => Ok(value),
    }
  }

  fn interpret_expr_call_builtin(
    &mut self,
    builtin: BuiltinFn,
    args: value::Args,
  ) -> Result<Value> {
    builtin(args)
  }

  fn interpret_expr_array(
    &mut self,
    elmts: &[Expr],
    span: Span,
  ) -> Result<Value> {
    let mut array = Array::new();

    for elmt in elmts {
      array.add_elmt(self.interpret_expr(elmt)?);
    }

    Ok(Value::array(array, span))
  }

  fn interpret_expr_array_access(
    &mut self,
    indexed: &Expr,
    index: &Expr,
    span: Span,
  ) -> Result<Value> {
    let indexed = self.interpret_expr(indexed)?;
    let index = self.interpret_expr(index)?;

    match (&indexed.kind, &index.kind) {
      (ValueKind::Array(array), ValueKind::Int(int)) => {
        self.interpret_expr_array_access_int(array, int, span)
      }
      _ => Err(ReportError::Eval(Eval::UnknownArrayAccess(
        span,
        indexed.to_string(),
        index.to_string(),
      ))),
    }
  }

  fn interpret_expr_array_access_int(
    &mut self,
    indexed: &[Value],
    index: &i64,
    span: Span,
  ) -> Result<Value> {
    match indexed.get(*index as usize) {
      Some(value) => Ok(value.to_owned()),
      _ => Err(ReportError::Eval(Eval::UnknownArrayAccessOperator(
        span,
        index.to_string(),
      ))),
    }
  }

  fn interpret_expr_struct(
    &mut self,
    strctr: &StructExpr,
    span: Span,
  ) -> Result<Value> {
    let mut record = HashMap::new();

    for (key, value) in strctr.pairs.iter() {
      let key = self.interpret_expr(key)?;
      let value = self.interpret_expr(value)?;
      let record_key = RecordKey::from(&key);

      record.insert(record_key, value);
    }

    Ok(Value::record(record, span))
  }

  fn interpret_expr_struct_access(
    &mut self,
    _structure: &Expr,
    _prop: &Expr,
    _span: Span,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_record(
    &mut self,
    pairs: &[(Expr, Expr)],
    span: Span,
  ) -> Result<Value> {
    let mut record = HashMap::new();

    for (key, value) in pairs {
      let key = self.interpret_expr(key)?;
      let value = self.interpret_expr(value)?;
      let record_key = StructExprKey::from(&key);

      record.insert(record_key, value);
    }

    Ok(Value::strctr_expr(record, span))
  }

  fn interpret_expr_record_access(
    &mut self,
    record: &Expr,
    prop: &Expr,
    span: Span,
  ) -> Result<Value> {
    let record = self.interpret_expr(record)?;
    let prop = self.interpret_expr(prop)?;

    match (record.kind, prop.kind) {
      (ValueKind::Record(record), ValueKind::Ident(prop)) => {
        self.interpret_expr_record_access_ident(record, &prop, span)
      }
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_record_access_ident(
    &mut self,
    record: HashMap<RecordKey, Value>,
    prop: &Symbol,
    span: Span,
  ) -> Result<Value> {
    match record.get(&RecordKey::Ident(*prop)) {
      Some(value) => Ok(value.to_owned()),
      _ => Err(ReportError::Eval(Eval::UnknownRecordAccessOperator(
        span,
        prop.to_string(),
      ))),
    }
  }

  fn interpret_expr_if_else(
    &mut self,
    condition: &Expr,
    consequence: &Block,
    maybe_alternative: &Option<Box<Expr>>,
    span: Span,
  ) -> Result<Value> {
    let condition = self.interpret_expr(condition)?;

    if condition.as_bool() {
      self.interpret_expr_block(consequence)
    } else {
      maybe_alternative
        .as_ref()
        .map(|alternative| self.interpret_expr(alternative))
        .unwrap_or(Ok(Value::unit(span)))
    }
  }

  fn interpret_expr_when(
    &mut self,
    condition: &Expr,
    consequence: &Expr,
    alternative: &Expr,
  ) -> Result<Value> {
    let condition = self.interpret_expr(condition)?;

    if condition.as_bool() {
      self.interpret_expr(consequence)
    } else {
      self.interpret_expr(alternative)
    }
  }

  fn interpret_expr_loop(&mut self, _body: &Block) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_while(
    &mut self,
    condition: &Expr,
    body: &Block,
  ) -> Result<Value> {
    let condition = self.interpret_expr(condition)?;

    while condition.as_bool() {
      self.interpret_block(body)?;
    }

    Ok(Value::UNIT)
  }

  fn interpret_expr_return(
    &mut self,
    maybe_expr: &Option<Box<Expr>>,
    span: Span,
  ) -> Result<Value> {
    match maybe_expr {
      Some(expr) => self.interpret_expr(expr),
      _ => Ok(Value::ret(Value::unit(span), span)),
    }
  }

  fn interpret_expr_break(
    &mut self,
    maybe_expr: &Option<Box<Expr>>,
    _span: Span,
  ) -> Result<Value> {
    match maybe_expr {
      Some(expr) => self.interpret_expr(expr),
      _ => todo!(), // break without expression.
    }
  }

  fn interpret_expr_continue(&mut self) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_var(&mut self, var: &Var) -> Result<Value> {
    self.interpret_var(var)
  }
}

/// ...
///
/// ## examples.
///
/// ```rs
/// ```
pub fn interpret(interpreter: &mut Interpreter, ast: &Ast) -> Result<Value> {
  interpreter.interpret(ast)
}
