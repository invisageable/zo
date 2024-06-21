//! ...

// todo #1 — at the moment, i'm not able to generate an executable using
// cranelift. the lack of understandable documentation is quite demotivating.
// @see — https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/index.md.

use super::clif::Function;

use zo_ast::ast::{
  Ast, BinOp, BinOpKind, Block, Expr, ExprKind, Ext, Fun, Item, ItemKind, Lit,
  LitKind, Pattern, PatternKind, Prototype, Stmt, StmtKind, TyAlias, UnOp,
  UnOpKind, Var,
};

use zo_core::interner::symbol::{Symbol, Symbolize};
use zo_core::interner::Interner;
use zo_core::reporter::Reporter;
use zo_core::Result;

use cranelift::frontend::FunctionBuilder;
use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, Value};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_object::ObjectModule;
use hashbrown::HashMap;

// todo #1.

pub(crate) struct Translator<'ast> {
  pub interner: &'ast mut Interner,
  pub reporter: &'ast mut Reporter,
  pub module: &'ast mut ObjectModule,
  pub builder: FunctionBuilder<'ast>,
  pub functions: HashMap<Symbol, Function>,
}

impl<'ast> Translator<'ast> {
  pub fn translate(mut self, ast: &Ast) -> Result<Value> {
    let mut value = Value::new(0usize);

    for stmt in ast.iter() {
      value = self.translate_stmt(stmt)?;
    }

    self.reporter.abort_if_has_errors();

    Ok(value)
  }

  fn translate_block(&mut self, block: &Block) -> Result<Value> {
    let mut value = Value::new(0usize);

    for stmt in block.iter() {
      value = self.translate_stmt(stmt)?;
    }

    Ok(value)
  }

  fn translate_item(&mut self, item: &Item) -> Result<Value> {
    match &item.kind {
      ItemKind::Fun(fun) => self.translate_item_fun(fun),
      _ => todo!(),
    }
  }

  fn translate_item_fun(&mut self, fun: &Fun) -> Result<Value> {
    let mut context = self.module.make_context();
    let signature = &mut context.func.signature;

    for _input in fun.prototype.inputs.iter() {
      signature.params.push(AbiParam::new(types::F64));
    }

    signature.returns.push(AbiParam::new(types::F64));

    let func_id = self.translate_prototype(&fun.prototype, Linkage::Export)?;

    let entry_block = self.builder.create_block();

    self
      .builder
      .append_block_params_for_function_params(entry_block);

    self.builder.switch_to_block(entry_block);
    self.builder.seal_block(entry_block);

    let value = self.translate_block(&fun.body)?;

    self.builder.ins().return_(&[value]);

    self.module.define_function(func_id, &mut context).unwrap();
    self.module.clear_context(&mut context);

    Ok(value)
  }

  fn translate_prototype(
    &mut self,
    prototype: &Prototype,
    linkage: Linkage,
  ) -> Result<FuncId> {
    let function_symbol = prototype.pattern.as_symbol();
    let inputs = &prototype.inputs;

    match self.functions.get(function_symbol) {
      Some(function) => {
        if function.defined {
          // returns reporter error.
        }

        if function.param_count != inputs.len() {
          // returns reporter error.
        }

        Ok(function.id)
      }
      None => {
        let mut signature = self.module.make_signature();

        for _parameter in inputs.iter() {
          signature.params.push(AbiParam::new(types::F64));
        }

        signature.returns.push(AbiParam::new(types::F64));

        let function_name = self.interner.lookup_ident(*function_symbol);

        let func_id = self
          .module
          .declare_function(&function_name, linkage, &signature)
          .unwrap();

        self.functions.insert(
          *function_symbol,
          Function {
            defined: false,
            id: func_id,
            param_count: inputs.len(),
          },
        );

        Ok(func_id)
      }
    }
  }

  fn translate_stmt(&mut self, stmt: &Stmt) -> Result<Value> {
    match &stmt.kind {
      StmtKind::Var(var) => self.translate_stmt_var(var),
      StmtKind::Item(item) => self.translate_stmt_item(item),
      StmtKind::Expr(expr) => self.translate_stmt_expr(expr),
    }
  }

  fn translate_stmt_var(&mut self, var: &Var) -> Result<Value> {
    self.translate_var(var)
  }

  fn translate_var(&mut self, _var: &Var) -> Result<Value> {
    todo!()
  }

  fn translate_stmt_item(&mut self, item: &Item) -> Result<Value> {
    self.translate_item(item)
  }

  fn translate_stmt_expr(&mut self, expr: &Expr) -> Result<Value> {
    self.translate_expr(expr)
  }

  fn translate_expr(&mut self, expr: &Expr) -> Result<Value> {
    match &expr.kind {
      _ => todo!(),
    }
  }
}
