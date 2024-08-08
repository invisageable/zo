use super::scope::ScopeMap;

use zo_ast::ast;
use zo_interner::interner::symbol::{Symbol, Symbolize};
use zo_interner::interner::Interner;
use zo_reporter::reporter::Reporter;
use zo_reporter::{error, Result};
use zo_session::session::Session;
use zo_value::builtin::BuiltinFn;
use zo_value::value::{Value, ValueKind};

use swisskit::span::Span;

/// The representation of an interpreter.
struct Interpreter<'ast> {
  /// A scope map — see also [`ScopeMap`].
  scope_map: ScopeMap,
  /// A flag for a break expression.
  breaking: bool,
  /// A flag for a break expression.
  continuing: bool,
  /// A loop counter.
  counter_loop: u32,
  /// An interner — see also [`Interner`] for more information.
  interner: &'ast mut Interner,
  /// A reporter — see also [`Reporter`] for more information.
  reporter: &'ast Reporter,
}

impl<'ast> Interpreter<'ast> {
  /// Creates a new interpreter.
  #[inline]
  fn new(interner: &'ast mut Interner, reporter: &'ast Reporter) -> Self {
    Self {
      scope_map: ScopeMap::new(),
      breaking: false,
      continuing: false,
      counter_loop: 0u32,
      interner,
      reporter,
    }
  }

  /// Interprets an AST.
  fn interpret(&mut self, ast: &ast::Ast) -> Result<Value> {
    let mut value = Value::UNIT;

    self.scope_map.scope_entry();

    for stmt in ast.iter() {
      value = match self.interpret_stmt(stmt) {
        Ok(value) => value,
        Err(error) => self.reporter.raise(error),
      };
    }

    self.scope_map.scope_exit();

    Ok(value)
  }

  /// Interprets an item statement.
  fn interpret_stmt_item(&mut self, item: &ast::Item) -> Result<Value> {
    match &item.kind {
      ast::ItemKind::Var(var) => self.interpret_item_var(var),
    }
  }

  /// Interprets a variable item.
  fn interpret_item_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_global_var(var)
  }

  /// Interprets a global variable.
  fn interpret_global_var(&mut self, var: &ast::Var) -> Result<Value> {
    let name = *var.pattern.as_symbol();
    let value = self.interpret_expr(&var.value)?;

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
  }

  /// Interprets a statement.
  fn interpret_stmt(&mut self, stmt: &ast::Stmt) -> Result<Value> {
    match &stmt.kind {
      ast::StmtKind::Var(var) => self.interpret_stmt_var(var),
      ast::StmtKind::Item(var) => self.interpret_stmt_item(var),
      ast::StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
    }
  }

  /// Interprets a local variable statement.
  fn interpret_stmt_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_var(var)
  }

  /// Interprets a variable.
  fn interpret_var(&mut self, var: &ast::Var) -> Result<Value> {
    let name = *var.pattern.as_symbol();
    let value = self.interpret_expr(&var.value)?;

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
  }

  /// Interprets an expression statement.
  fn interpret_stmt_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  /// Interprets an expression.
  fn interpret_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => {
        self.interpret_expr_unop(unop, rhs, expr.span)
      }
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs, expr.span)
      }
      ast::ExprKind::Assign(assignee, value) => {
        self.interpret_expr_assign(assignee, value)
      }
      ast::ExprKind::AssignOp(binop, assignee, value) => {
        self.interpret_expr_assign_op(binop, assignee, value, expr.span)
      }
      ast::ExprKind::Array(elmts) => {
        self.interpret_expr_array(elmts, expr.span)
      }
      ast::ExprKind::ArrayAccess(indexed, index) => {
        self.interpret_expr_array_access(indexed, index, expr.span)
      }
      ast::ExprKind::Tuple(elmts) => {
        self.interpret_expr_tuple(elmts, expr.span)
      }
      ast::ExprKind::TupleAccess(indexed, index) => {
        self.interpret_expr_tuple_access(indexed, index, expr.span)
      }
      ast::ExprKind::IfElse(condition, consequence, maybe_alternative) => self
        .interpret_expr_if_else(
          condition,
          consequence,
          maybe_alternative,
          expr.span,
        ),
      ast::ExprKind::When(condition, consequence, alternative) => {
        self.interpret_expr_when(condition, consequence, alternative)
      }
      ast::ExprKind::Loop(body) => self.interpret_expr_loop(body),
      ast::ExprKind::While(condition, body) => {
        self.interpret_expr_while(condition, body)
      }
      ast::ExprKind::Return(maybe_expr) => {
        self.interpret_expr_return(maybe_expr, expr.span)
      }
      ast::ExprKind::Break(maybe_expr) => {
        self.interpret_expr_break(maybe_expr, expr.span)
      }
      ast::ExprKind::Continue => self.interpret_expr_continue(expr.span),
      ast::ExprKind::Var(var) => self.interpret_expr_var(var),
      ast::ExprKind::Closure(prototype, block) => {
        self.interpret_expr_closure(prototype, block, expr.span)
      }
      ast::ExprKind::Call(callee, args) => {
        self.interpret_expr_call(callee, args, expr.span)
      }
    }
  }

  /// Interprets a literal expression.
  fn interpret_expr_lit(&mut self, lit: &ast::Lit) -> Result<Value> {
    match &lit.kind {
      ast::LitKind::Int(sym, _) => self.interpret_expr_lit_int(sym, lit.span),
      ast::LitKind::Float(sym) => self.interpret_expr_lit_float(sym, lit.span),
      ast::LitKind::Ident(sym) => self.interpret_expr_lit_ident(sym, lit.span),
      ast::LitKind::Bool(boolean) => {
        self.interpret_expr_lit_bool(boolean, lit.span)
      }
      _ => todo!(),
    }
  }

  /// Interprets an integer literal expression.
  fn interpret_expr_lit_int(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let int = self.interner.lookup_int(**sym as usize);

    Ok(Value::int(int, span))
  }

  /// Interprets a float literal expression.
  fn interpret_expr_lit_float(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    let float = self.interner.lookup_float(**sym as usize);

    Ok(Value::float(float, span))
  }

  /// Interprets an identifier literal expression.
  fn interpret_expr_lit_ident(
    &mut self,
    sym: &Symbol,
    span: Span,
  ) -> Result<Value> {
    if let Some(var) = self.scope_map.var(sym) {
      return Ok(var.to_owned());
    } else if let Some(fun) = self.scope_map.fun(sym) {
      return Ok(fun.to_owned());
    }

    let name = self.interner.lookup(**sym);

    Err(error::eval::not_found_ident(span, name))
  }

  /// Interprets a boolean literal expression.
  fn interpret_expr_lit_bool(
    &mut self,
    boolean: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(*boolean, span))
  }

  /// Interprets an unary operation expression.
  fn interpret_expr_unop(
    &mut self,
    unop: &ast::UnOp,
    rhs: &ast::Expr,
    span: Span,
  ) -> Result<Value> {
    let value = self.interpret_expr(rhs)?;

    match unop.kind {
      ast::UnOpKind::Neg => self.interpret_expr_unop_neg(value, span),
      ast::UnOpKind::Not => self.interpret_expr_unop_not(value, span),
    }
  }

  /// Interprets a negative unary operation expression.
  fn interpret_expr_unop_neg(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    Ok(match rhs.kind {
      ValueKind::Int(int) => Value::int(-int, span),
      ValueKind::Float(float) => Value::float(-float, span),
      _ => return Err(error::eval::unknown_unop(span, rhs)),
    })
  }

  /// Interprets a logical NOT unary operation expression
  fn interpret_expr_unop_not(
    &mut self,
    rhs: Value,
    span: Span,
  ) -> Result<Value> {
    Ok(Value::bool(!rhs.as_bool(), span))
  }

  /// Interprets a binary operation expression.
  fn interpret_expr_binop(
    &mut self,
    binop: &ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
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
      _ => Err(error::eval::unknown_binop(span, *binop)),
    }
  }

  /// Interprets a binary operation expression for integers.
  fn interpret_expr_binop_int(
    &mut self,
    binop: &ast::BinOp,
    lhs: &i64,
    rhs: &i64,
    span: Span,
  ) -> Result<Value> {
    println!("{lhs} {binop:?} {rhs}");

    Ok(match binop.kind {
      ast::BinOpKind::Add => Value::int(lhs + rhs, span),
      ast::BinOpKind::Sub => Value::int(lhs - rhs, span),
      ast::BinOpKind::Mul => Value::int(lhs * rhs, span),
      ast::BinOpKind::Div => Value::int(lhs / rhs, span),
      ast::BinOpKind::Rem => Value::int(lhs % rhs, span),
      ast::BinOpKind::Shl => Value::int(lhs << rhs, span),
      ast::BinOpKind::Shr => Value::int(lhs >> rhs, span),
      ast::BinOpKind::Lt => Value::bool(lhs < rhs, span),
      ast::BinOpKind::Gt => Value::bool(lhs > rhs, span),
      ast::BinOpKind::Le => Value::bool(lhs <= rhs, span),
      ast::BinOpKind::Ge => Value::bool(lhs >= rhs, span),
      ast::BinOpKind::Eq => Value::bool(lhs == rhs, span),
      ast::BinOpKind::Ne => Value::bool(lhs != rhs, span),
      _ => return Err(error::eval::unknown_binop(span, *binop)),
    })
  }

  /// Interprets a binary operation expression for floats.
  fn interpret_expr_binop_float(
    &mut self,
    binop: &ast::BinOp,
    lhs: &f64,
    rhs: &f64,
    span: Span,
  ) -> Result<Value> {
    Ok(match binop.kind {
      ast::BinOpKind::Add => Value::float(lhs + rhs, span),
      ast::BinOpKind::Sub => Value::float(lhs - rhs, span),
      ast::BinOpKind::Mul => Value::float(lhs * rhs, span),
      ast::BinOpKind::Div => Value::float(lhs / rhs, span),
      ast::BinOpKind::Rem => Value::float(lhs % rhs, span),
      ast::BinOpKind::Lt => Value::bool(lhs < rhs, span),
      ast::BinOpKind::Gt => Value::bool(lhs > rhs, span),
      ast::BinOpKind::Le => Value::bool(lhs <= rhs, span),
      ast::BinOpKind::Ge => Value::bool(lhs >= rhs, span),
      ast::BinOpKind::Eq => Value::bool(lhs == rhs, span),
      ast::BinOpKind::Ne => Value::bool(lhs != rhs, span),
      _ => return Err(error::eval::unknown_binop(span, *binop)),
    })
  }

  /// Interprets a binary operation expression for booleans.
  fn interpret_expr_binop_bool(
    &mut self,
    binop: &ast::BinOp,
    lhs: &bool,
    rhs: &bool,
    span: Span,
  ) -> Result<Value> {
    Ok(match binop.kind {
      ast::BinOpKind::Eq => Value::bool(lhs == rhs, span),
      ast::BinOpKind::Ne => Value::bool(lhs != rhs, span),
      ast::BinOpKind::And => Value::bool(*lhs && *rhs, span),
      ast::BinOpKind::Or => Value::bool(*lhs || *rhs, span),
      ast::BinOpKind::BitAnd => Value::bool(*lhs & *rhs, span),
      ast::BinOpKind::BitOr => Value::bool(*lhs | *rhs, span),
      ast::BinOpKind::BitXor => Value::bool(*lhs ^ *rhs, span),
      _ => return Err(error::eval::unknown_binop(span, *binop)),
    })
  }

  /// Interprets an assignment expression.
  fn interpret_expr_assign(
    &mut self,
    assignee: &ast::Expr,
    value: &ast::Expr,
  ) -> Result<Value> {
    let name = *assignee.as_symbol();
    let value = self.interpret_expr(value)?;

    self.scope_map.add_var(name, value.clone())?;

    Ok(value)
  }

  /// Interprets an assignment operator expression.
  fn interpret_expr_assign_op(
    &mut self,
    binop: &ast::BinOp,
    assignee: &ast::Expr,
    value: &ast::Expr,
    span: Span,
  ) -> Result<Value> {
    let name = assignee.as_symbol();

    let lhs = match self.scope_map.var(name) {
      Some(value) => value.to_owned(),
      None => return Err(error::eval::not_found_var(span, *name)),
    };

    self.scope_map.update_var(*name, lhs.clone())?;

    let rhs = self.interpret_expr(value)?;

    Ok(match binop.kind {
      ast::BinOpKind::Add => lhs + rhs,
      ast::BinOpKind::Sub => lhs - rhs,
      ast::BinOpKind::Mul => lhs * rhs,
      ast::BinOpKind::Div => lhs / rhs,
      ast::BinOpKind::Rem => lhs % rhs,
      // todo — should be `unknown assignop` error instead.
      _ => return Err(error::eval::unknown_binop(span, *binop)),
    })
  }

  /// Interprets an array expression.
  fn interpret_expr_array(
    &mut self,
    elmts: &[ast::Expr],
    span: Span,
  ) -> Result<Value> {
    let mut array = Vec::with_capacity(elmts.len());

    for elmt in elmts {
      array.push(self.interpret_expr(elmt)?);
    }

    Ok(Value::array(array, span))
  }

  /// Interprets an array access expression.
  fn interpret_expr_array_access(
    &mut self,
    indexed: &ast::Expr,
    index: &ast::Expr,
    span: Span,
  ) -> Result<Value> {
    let indexed = self.interpret_expr(indexed)?;
    let index = self.interpret_expr(index)?;

    if let (ValueKind::Array(array), ValueKind::Int(ref int)) =
      (&indexed.kind, &index.kind)
    {
      let index = *int;

      return match array.get(index as usize) {
        Some(value) => Ok(value.to_owned()),
        None => Err(error::eval::out_of_bound_array(span, index)),
      };
    }

    Err(error::eval::invalid_array_access(span, indexed, index))
  }

  /// Interprets a tuple expression.
  fn interpret_expr_tuple(
    &mut self,
    elmts: &[ast::Expr],
    span: Span,
  ) -> Result<Value> {
    let mut tuple = Vec::with_capacity(elmts.len());

    for elmt in elmts {
      tuple.push(self.interpret_expr(elmt)?);
    }

    Ok(Value::tuple(tuple, span))
  }

  /// Interprets a tuple expression.
  fn interpret_expr_tuple_access(
    &mut self,
    indexed: &ast::Expr,
    index: &ast::Expr,
    span: Span,
  ) -> Result<Value> {
    let indexed = self.interpret_expr(indexed)?;
    let index = self.interpret_expr(index)?;

    if let (ValueKind::Tuple(tuple), ValueKind::Int(ref int)) =
      (&indexed.kind, &index.kind)
    {
      let index = *int;

      return match tuple.get(index as usize) {
        Some(value) => Ok(value.to_owned()),
        None => Err(error::eval::out_of_bound_tuple(span, index)),
      };
    }

    Err(error::eval::invalid_tuple_access(span, indexed, index))
  }

  /// Interprets an if else condition expression.
  fn interpret_expr_if_else(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Block,
    maybe_alternative: &Option<Box<ast::Expr>>,
    span: Span,
  ) -> Result<Value> {
    let condition = self.interpret_expr(condition)?;

    if condition.as_bool() {
      self.interpret_block(consequence)
    } else {
      maybe_alternative
        .as_ref()
        .map(|alternative| self.interpret_expr(alternative))
        .unwrap_or(Ok(Value::unit(span)))
    }
  }

  /// Interprets a block.
  fn interpret_block(&mut self, block: &ast::Block) -> Result<Value> {
    let mut value = Value::UNIT;

    for stmt in block.iter() {
      value = self.interpret_stmt(stmt)?;

      if let ValueKind::Return(value) = value.kind {
        return Ok(*value);
      }
    }

    Ok(value)
  }

  /// Interprets a ternary condition expression.
  fn interpret_expr_when(
    &mut self,
    condition: &ast::Expr,
    consequence: &ast::Expr,
    alternative: &ast::Expr,
  ) -> Result<Value> {
    let condition = self.interpret_expr(condition)?;

    if condition.as_bool() {
      self.interpret_expr(consequence)
    } else {
      self.interpret_expr(alternative)
    }
  }

  /// Interprets a loop expression.
  fn interpret_expr_loop(&mut self, _body: &ast::Block) -> Result<Value> {
    todo!()
  }

  /// Interprets a while expression.
  fn interpret_expr_while(
    &mut self,
    condition: &ast::Expr,
    body: &ast::Block,
  ) -> Result<Value> {
    let mut value = Value::UNIT;

    self.counter_loop += 1;

    while as_bool(self.interpret_expr(condition)?) {
      value = self.interpret_block(body)?;

      match &value.kind {
        ValueKind::Return(value) => return Ok(*value.to_owned()),
        ValueKind::Break(value) => match value.kind {
          ValueKind::Unit => break,
          _ => {
            return Err(error::eval::break_in_while_loop_with_value(value.span))
          }
        },
        ValueKind::Continue => continue,
        _ => {}
      }
    }

    self.counter_loop -= 1;

    Ok(value)
  }

  /// Interprets a return expression.
  fn interpret_expr_return(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
    span: Span,
  ) -> Result<Value> {
    match maybe_expr {
      Some(expr) => self.interpret_expr(expr),
      None => Ok(Value::ret(Value::unit(span), span)),
    }
  }

  /// Interprets a break expression.
  fn interpret_expr_break(
    &mut self,
    maybe_expr: &Option<Box<ast::Expr>>,
    span: Span,
  ) -> Result<Value> {
    if self.counter_loop == 0 {
      return Err(error::eval::out_of_loop(span, "break"));
    }

    self.breaking = true;

    match maybe_expr {
      Some(expr) => self.interpret_expr(expr),
      None => Ok(Value::brk(Box::new(Value::unit(span)), span)),
    }
  }

  /// Interprets a continue expression.
  fn interpret_expr_continue(&mut self, span: Span) -> Result<Value> {
    if self.counter_loop == 0 {
      return Err(error::eval::out_of_loop(span, "continue"));
    }

    self.continuing = true;

    Ok(Value::ctn(span))
  }

  /// Interprets a local variable expression.
  fn interpret_expr_var(&mut self, var: &ast::Var) -> Result<Value> {
    self.interpret_var(var)
  }

  /// Interprets a closure expression.
  fn interpret_expr_closure(
    &mut self,
    prototype: &ast::Prototype,
    block: &ast::Block,
    span: Span,
  ) -> Result<Value> {
    // needs work.
    let name = *prototype.as_symbol();
    let value = Value::closure(prototype.to_owned(), block.to_owned(), span);

    self.scope_map.add_fun(name, value.to_owned())?;

    Ok(value)
  }

  /// Interprets a call expression.
  fn interpret_expr_call(
    &mut self,
    callee: &ast::Expr,
    args: &[ast::Expr],
    _span: Span,
  ) -> Result<Value> {
    let callee = self.interpret_expr(callee)?;
    let args = self.interpret_args(args)?;

    match callee.kind {
      ValueKind::Closure(prototype, block) => {
        self.interpret_expr_call_fn(prototype, block, args)
      }
      ValueKind::Builtin(builtin) => {
        self.interpret_expr_call_builtin(builtin, args)
      }
      _ => panic!(),
    }
  }

  /// Interprets arguments.
  fn interpret_args(&mut self, args: &[ast::Expr]) -> Result<Vec<Value>> {
    let mut values = Vec::with_capacity(0usize);

    for arg in args.iter() {
      values.push(self.interpret_expr(arg)?);
    }

    Ok(values)
  }

  /// Interprets a call function exppression.
  fn interpret_expr_call_fn(
    &mut self,
    prototype: ast::Prototype,
    block: ast::Block,
    args: Vec<Value>,
  ) -> Result<Value> {
    if prototype.inputs.len() != args.len() {
      panic!()
    }

    self.scope_map.scope_entry();

    for (idx, input) in prototype.inputs.iter().enumerate() {
      let value = args.get(idx).unwrap();
      let name = *input.as_symbol();

      self.scope_map.add_var(name, value.to_owned())?;
    }

    let value = self.interpret_block(&block)?;

    self.scope_map.scope_exit();

    match value.kind {
      ValueKind::Return(value) => Ok(*value),
      _ => Ok(value),
    }
  }

  /// Interprets call builtin function expression.
  fn interpret_expr_call_builtin(
    &mut self,
    builtin: BuiltinFn,
    values: Vec<Value>,
  ) -> Result<Value> {
    builtin(values)
  }
}

fn as_bool(value: Value) -> bool {
  match value.kind {
    ValueKind::Unit => false,
    ValueKind::Bool(boolean) => boolean,
    _ => true,
  }
}

/// Interprets an AST — see also [`Interpreter::interpret`].
///
/// #### examples.
///
/// ```ignore
/// use zo_ast::ast::Ast;
/// use zo_interpreter_zo::interpreter;
/// use zo_session::session::Session;
/// use zo_value::value::Value;
///
/// let mut session = Session::default();
/// let ast = Ast::new();
/// let value = interpreter::interpret(&mut session, &ast);
///
/// assert_eq!(value, Value::UNIT);
/// ```
pub fn interpret(session: &mut Session, ast: &ast::Ast) -> Result<Value> {
  Interpreter::new(&mut session.interner, &session.reporter).interpret(ast)
}
