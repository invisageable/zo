//! ...

use super::scope::Scope;

use zo_ast::ast::{
  Arg, Args, Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Lit, LitKind,
  Prototype, UnOp, UnOpKind, Var,
};

use zo_value::value::{self, Value, ValueKind};

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::Result;

use smol_str::{SmolStr, ToSmolStr};

pub struct Interpreter<'ast> {
  interner: &'ast mut Interner,
  reporter: &'ast Reporter,
  scope: Scope,
}

impl<'ast> Interpreter<'ast> {
  #[inline]
  pub fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self {
      interner,
      reporter,
      scope: Scope::new(),
    }
  }

  pub fn interpret(&mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    for expr in &ast.exprs {
      value = self.interpret_expr(expr)?;

      if let ValueKind::Return(value) = value.kind {
        return Ok(*value);
      }
    }

    self.reporter.abort_if_has_errors();

    Ok(value)
  }

  fn interpret_expr(&mut self, expr: &Expr) -> Result<Value> {
    match &expr.kind {
      ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ExprKind::UnOp(unop, rhs) => self.interpret_expr_unop(unop, rhs),
      ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs)
      }
      ExprKind::Assign(_assignee, _value) => todo!(),
      ExprKind::AssignOp(_binop, _assignee, _value) => todo!(),
      ExprKind::Block(block) => self.interpret_expr_block(block),
      ExprKind::Fn(prototype, block) => {
        self.interpret_expr_fn(prototype, block)
      }
      ExprKind::Call(callee, args) => self.interpret_expr_call(callee, args),
      ExprKind::Array(elmts) => self.interpret_expr_array(elmts),
      ExprKind::ArrayAccess(indexed, index) => {
        self.interpret_expr_array_access(indexed, index)
      }
      ExprKind::IfElse(condition, consequence, maybe_alternative) => {
        self.interpret_expr_if_else(condition, consequence, maybe_alternative)
      }
      ExprKind::When(condition, consequence, alternative) => {
        self.interpret_expr_when(condition, consequence, alternative)
      }
      ExprKind::Loop(body) => self.interpret_expr_loop(body),
      ExprKind::While(condition, body) => {
        self.interpret_expr_while(condition, body)
      }
      ExprKind::Return(maybe_expr) => self.interpret_expr_return(maybe_expr),
      ExprKind::Break(maybe_expr) => self.interpret_expr_break(maybe_expr),
      ExprKind::Continue => self.interpret_expr_continue(),
      ExprKind::Var(var) => self.interpret_expr_var(var),
    }
  }

  fn interpret_expr_lit(&mut self, lit: &Lit) -> Result<Value> {
    match &lit.kind {
      LitKind::Int(symbol) => self.interpret_expr_lit_int(symbol),
      LitKind::Float(symbol) => self.interpret_expr_lit_float(symbol),
      LitKind::Ident(symbol) => self.interpret_expr_lit_ident(symbol),
      LitKind::Bool(symbol) => self.interpret_expr_lit_bool(symbol),
      LitKind::Char(symbol) => self.interpret_expr_lit_char(symbol),
      LitKind::Str(symbol) => self.interpret_expr_lit_str(symbol),
    }
  }

  fn interpret_expr_lit_int(&mut self, symbol: &Symbol) -> Result<Value> {
    let int = self.interner.lookup_int(symbol);

    Ok(Value::int(int))
  }

  fn interpret_expr_lit_float(&mut self, symbol: &Symbol) -> Result<Value> {
    let float = self.interner.lookup_float(symbol);

    Ok(Value::float(float))
  }

  fn interpret_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<Value> {
    if let Some(var) = self.scope.var(symbol) {
      return Ok(var.clone());
    } else if let Some(fun) = self.scope.fun(symbol) {
      return Ok(fun.clone());
    }

    let _ident = self.interner.lookup_ident(symbol);

    panic!() // returns reporter error.
  }

  fn interpret_expr_lit_bool(&mut self, boolean: &bool) -> Result<Value> {
    Ok(Value::bool(*boolean))
  }

  fn interpret_expr_lit_char(&mut self, symbol: &Symbol) -> Result<Value> {
    let ch = self.interner.lookup_char(symbol);

    Ok(Value::char(ch))
  }

  fn interpret_expr_lit_str(&mut self, symbol: &Symbol) -> Result<Value> {
    let string = self.interner.lookup_str(symbol);

    Ok(Value::str(string.to_smolstr()))
  }

  fn interpret_expr_unop(&mut self, unop: &UnOp, rhs: &Expr) -> Result<Value> {
    let value = self.interpret_expr(rhs)?;

    match &unop.kind {
      UnOpKind::Neg => self.interpret_expr_unop_neg(value),
      UnOpKind::Not => self.interpret_expr_unop_not(value),
    }
  }

  fn interpret_expr_unop_neg(&mut self, rhs: Value) -> Result<Value> {
    match rhs.kind {
      ValueKind::Int(int) => Ok(Value::int(int)),
      ValueKind::Float(float) => Ok(Value::float(float)),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_unop_not(&mut self, rhs: Value) -> Result<Value> {
    Ok(Value::bool(!rhs.as_bool()))
  }

  fn interpret_expr_binop(
    &mut self,
    binop: &BinOp,
    lhs: &Expr,
    rhs: &Expr,
  ) -> Result<Value> {
    let lhs = self.interpret_expr(lhs)?;
    let rhs = self.interpret_expr(rhs)?;

    match (&lhs.kind, &rhs.kind) {
      (ValueKind::Int(lhs), ValueKind::Int(rhs)) => {
        self.interpret_expr_binop_int(binop, lhs, rhs)
      }
      (ValueKind::Float(lhs), ValueKind::Float(rhs)) => {
        self.interpret_expr_binop_float(binop, lhs, rhs)
      }
      (ValueKind::Bool(lhs), ValueKind::Bool(rhs)) => {
        self.interpret_expr_binop_bool(binop, lhs, rhs)
      }
      (ValueKind::Str(lhs), ValueKind::Str(rhs)) => {
        self.interpret_expr_binop_str(binop, lhs, rhs)
      }
      _ => panic!(), // returns reporter error.
    }
  }

  /*
    Rem,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    Shl,
    Shr,
  */

  fn interpret_expr_binop_int(
    &mut self,
    binop: &BinOp,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    match &binop.kind {
      BinOpKind::Add => self.interpret_expr_binop_int_add(lhs, rhs),
      BinOpKind::Sub => self.interpret_expr_binop_int_sub(lhs, rhs),
      BinOpKind::Mul => self.interpret_expr_binop_int_mul(lhs, rhs),
      BinOpKind::Div => self.interpret_expr_binop_int_div(lhs, rhs),
      BinOpKind::Rem => self.interpret_expr_binop_int_rem(lhs, rhs),
      BinOpKind::Lt => self.interpret_expr_binop_int_lt(lhs, rhs),
      BinOpKind::Gt => self.interpret_expr_binop_int_gt(lhs, rhs),
      BinOpKind::Le => self.interpret_expr_binop_int_le(lhs, rhs),
      BinOpKind::Ge => self.interpret_expr_binop_int_ge(lhs, rhs),
      BinOpKind::Eq => self.interpret_expr_binop_int_eq(lhs, rhs),
      BinOpKind::Ne => self.interpret_expr_binop_int_ne(lhs, rhs),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_binop_int_add(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::int(lhs + rhs))
  }

  fn interpret_expr_binop_int_sub(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::int(lhs - rhs))
  }

  fn interpret_expr_binop_int_mul(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::int(lhs * rhs))
  }

  fn interpret_expr_binop_int_div(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::int(lhs / rhs))
  }

  fn interpret_expr_binop_int_rem(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::int(lhs % rhs))
  }

  fn interpret_expr_binop_int_lt(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs < rhs))
  }

  fn interpret_expr_binop_int_gt(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs > rhs))
  }

  fn interpret_expr_binop_int_le(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs <= rhs))
  }

  fn interpret_expr_binop_int_ge(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs >= rhs))
  }

  fn interpret_expr_binop_int_eq(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs == rhs))
  }

  fn interpret_expr_binop_int_ne(
    &mut self,
    lhs: &i64,
    rhs: &i64,
  ) -> Result<Value> {
    Ok(Value::bool(lhs != rhs))
  }

  fn interpret_expr_binop_float(
    &mut self,
    binop: &BinOp,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    match &binop.kind {
      BinOpKind::Add => self.interpret_expr_binop_float_add(lhs, rhs),
      BinOpKind::Sub => self.interpret_expr_binop_float_sub(lhs, rhs),
      BinOpKind::Mul => self.interpret_expr_binop_float_mul(lhs, rhs),
      BinOpKind::Div => self.interpret_expr_binop_float_div(lhs, rhs),
      BinOpKind::Rem => self.interpret_expr_binop_float_rem(lhs, rhs),
      BinOpKind::Lt => self.interpret_expr_binop_float_lt(lhs, rhs),
      BinOpKind::Gt => self.interpret_expr_binop_float_gt(lhs, rhs),
      BinOpKind::Le => self.interpret_expr_binop_float_le(lhs, rhs),
      BinOpKind::Ge => self.interpret_expr_binop_float_ge(lhs, rhs),
      BinOpKind::Eq => self.interpret_expr_binop_float_eq(lhs, rhs),
      BinOpKind::Ne => self.interpret_expr_binop_float_ne(lhs, rhs),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_binop_float_add(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::float(lhs + rhs))
  }

  fn interpret_expr_binop_float_sub(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::float(lhs - rhs))
  }

  fn interpret_expr_binop_float_mul(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::float(lhs * rhs))
  }

  fn interpret_expr_binop_float_div(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::float(lhs / rhs))
  }

  fn interpret_expr_binop_float_rem(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::float(lhs % rhs))
  }

  fn interpret_expr_binop_float_lt(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs < *rhs))
  }

  fn interpret_expr_binop_float_gt(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs > *rhs))
  }

  fn interpret_expr_binop_float_le(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs <= *rhs))
  }

  fn interpret_expr_binop_float_ge(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs >= *rhs))
  }

  fn interpret_expr_binop_float_eq(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs == *rhs))
  }

  fn interpret_expr_binop_float_ne(
    &mut self,
    lhs: &f64,
    rhs: &f64,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs != *rhs))
  }

  fn interpret_expr_binop_bool(
    &mut self,
    binop: &BinOp,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    match &binop.kind {
      BinOpKind::And => self.interpret_expr_binop_bool_and(lhs, rhs),
      BinOpKind::Or => self.interpret_expr_binop_bool_or(lhs, rhs),
      BinOpKind::BitAnd => self.interpret_expr_binop_bool_bit_and(lhs, rhs),
      BinOpKind::BitOr => self.interpret_expr_binop_bool_bit_or(lhs, rhs),
      BinOpKind::BitXor => self.interpret_expr_binop_bool_bit_xor(lhs, rhs),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_binop_bool_and(
    &mut self,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs && *rhs))
  }

  fn interpret_expr_binop_bool_or(
    &mut self,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs || *rhs))
  }

  fn interpret_expr_binop_bool_bit_and(
    &mut self,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs & *rhs))
  }

  fn interpret_expr_binop_bool_bit_or(
    &mut self,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs | *rhs))
  }

  fn interpret_expr_binop_bool_bit_xor(
    &mut self,
    lhs: &bool,
    rhs: &bool,
  ) -> Result<Value> {
    Ok(Value::bool(*lhs ^ *rhs))
  }

  fn interpret_expr_binop_str(
    &mut self,
    binop: &BinOp,
    lhs: &SmolStr,
    rhs: &SmolStr,
  ) -> Result<Value> {
    match &binop.kind {
      BinOpKind::And => self.interpret_expr_binop_str_and(lhs, rhs),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_expr_binop_str_and(
    &mut self,
    lhs: &SmolStr,
    rhs: &SmolStr,
  ) -> Result<Value> {
    let string = format!("{lhs}{rhs}");

    Ok(Value::str(string.to_smolstr()))
  }

  fn interpret_expr_block(&mut self, block: &Block) -> Result<Value> {
    let mut value = Value::UNIT;

    for expr in &block.exprs {
      value = self.interpret_expr(expr)?;

      if let ValueKind::Return(value) = value.kind {
        return Ok(*value);
      }
    }

    Ok(value)
  }

  fn interpret_expr_fn(
    &mut self,
    prototype: &Prototype,
    block: &Block,
  ) -> Result<Value> {
    Ok(Value::fun(prototype.clone(), block.clone()))
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
      ValueKind::Builtin(fun) => self.interpret_expr_call_builtin(fun),
      _ => panic!(), // returns reporter error.
    }
  }

  fn interpret_args(&mut self, args: &Args) -> Result<value::Args> {
    let mut args_new = value::Args::new();

    if args.is_empty() {
      return Ok(args_new);
    }

    for arg in &args.0 {
      args_new.add_arg(self.interpret_arg(arg)?);
    }

    todo!()
  }

  fn interpret_arg(&mut self, arg: &Arg) -> Result<value::Arg> {
    let value = self.interpret_expr(&arg.expr)?;

    Ok(value::Arg { value })
  }

  fn interpret_expr_call_fn(
    &mut self,
    prototype: Prototype,
    block: Block,
    args: value::Args,
  ) -> Result<Value> {
    if prototype.inputs.len() != args.len() {
      panic!() // returns reporter error.
    }

    let _value = self.interpret_expr_block(&block)?;

    todo!()
  }

  fn interpret_expr_call_builtin(&mut self, _fun: ()) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_array(&mut self, _elmts: &[Expr]) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_array_access(
    &mut self,
    _indexed: &Expr,
    _index: &Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_if_else(
    &mut self,
    _condition: &Expr,
    _consequence: &Block,
    _maybe_alternative: &Option<Box<Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_when(
    &mut self,
    _condition: &Expr,
    _consequence: &Expr,
    _maybe_alternative: &Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_loop(&mut self, _body: &Block) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_while(
    &mut self,
    _condition: &Expr,
    _body: &Block,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_return(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_break(
    &mut self,
    _maybe_expr: &Option<Box<Expr>>,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_continue(&mut self) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_var(&mut self, _var: &Var) -> Result<Value> {
    todo!()
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
