#![allow(dead_code)]

use zhoo_ast::ast;
use zhoo_session::session::Session;

use zo_core::interner::symbol::Symbol;
use zo_core::interner::Interner;
use zo_core::reporter::report::eval::Eval;
use zo_core::Result;

use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
// use cranelift::prelude::types;
use cranelift::prelude::Configurable;
use cranelift_codegen::entity::EntityRef;
// use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::ir::Value;
use cranelift_codegen::settings;
use cranelift_codegen::Context;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};

struct Interpreter<'program> {
  interner: &'program mut Interner,
  ctx: Context,
  fun_ctx: FunctionBuilderContext,
  data_description: DataDescription,
  module: JITModule,
}

impl<'program> Interpreter<'program> {
  fn new(interner: &'program mut Interner) -> Result<Self> {
    let mut flag_builder = settings::builder();

    // inconsistent flag.
    flag_builder
      .set("use_colocated_libcalls", "false")
      .map_err(Eval::not_configurable)?;

    // inconsistent flag.
    flag_builder
      .set("is_pic", "false")
      .map_err(Eval::not_configurable)?;

    let isa_builder =
      cranelift_native::builder().map_err(Eval::host_machine)?;

    // target ISA not configurable.
    let isa = isa_builder
      .finish(settings::Flags::new(flag_builder))
      .map_err(Eval::not_configurable)?;

    let builder =
      JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

    let module = JITModule::new(builder);

    Ok(Self {
      interner,
      ctx: module.make_context(),
      fun_ctx: FunctionBuilderContext::new(),
      data_description: DataDescription::new(),
      module,
    })
  }

  fn interpret(&mut self, program: &ast::Program) -> Result<*const u8> {
    let builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.fun_ctx);

    let mut translator = Translator::new(builder, &self.module);

    translator.translate(program)?;

    for item in &program.items {
      self.interpret_item(item)?;
    }

    // IncompatibleDeclaration.
    let id = self
      .module
      .declare_function("main", Linkage::Export, &self.ctx.func.signature)
      .unwrap();

    // DuplicateDefinition.
    self.module.define_function(id, &mut self.ctx).unwrap();
    self.module.clear_context(&mut self.ctx);

    // InvalidImportDefinition.
    self.module.finalize_definitions().unwrap();

    // We can now retrieve a pointer to the machine code.
    let code = self.module.get_finalized_function(id);

    Ok(code)
  }

  fn interpret_item(&mut self, item: &ast::Item) -> Result<Value> {
    match &item.kind {
      ast::ItemKind::Pack(pack) => self.interpret_item_pack(pack),
      ast::ItemKind::Load(load) => self.interpret_item_load(load),
      ast::ItemKind::Var(var) => self.interpret_item_var(var),
      ast::ItemKind::TyAlias(ty_alias) => {
        self.interpret_item_ty_alias(ty_alias)
      }
      ast::ItemKind::Ext(ext) => self.interpret_item_ext(ext),
      ast::ItemKind::Abstract(abstr) => self.interpret_item_abstract(abstr),
      ast::ItemKind::Enum(enumeration) => self.interpret_item_enum(enumeration),
      ast::ItemKind::Struct(structure) => self.interpret_item_struct(structure),
      ast::ItemKind::Apply(apply) => self.interpret_item_apply(apply),
      ast::ItemKind::Fun(fun) => self.interpret_item_fun(fun),
    }
  }

  fn interpret_item_pack(&mut self, _pack: &ast::Pack) -> Result<Value> {
    todo!()
  }

  fn interpret_item_load(&mut self, _load: &ast::Load) -> Result<Value> {
    todo!()
  }

  fn interpret_item_var(&mut self, _var: &ast::Var) -> Result<Value> {
    todo!()
  }

  fn interpret_item_ty_alias(
    &mut self,
    _ty_alias: &ast::TyAlias,
  ) -> Result<Value> {
    todo!()
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

  fn interpret_item_struct(
    &mut self,
    _structure: &ast::Struct,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_item_apply(&mut self, _apply: &ast::Apply) -> Result<Value> {
    todo!()
  }

  fn interpret_item_fun(&mut self, fun: &ast::Fun) -> Result<Value> {
    self.interpret_prototype(&fun.prototype)?;
    self.interpret_block(&fun.body)
  }

  fn interpret_prototype(
    &mut self,
    prototype: &ast::Prototype,
  ) -> Result<Value> {
    let _name = self.interpret_pattern(&prototype.pattern)?;
    let _inputs = self.interpret_inputs(&prototype.inputs)?;
    let _output_py = self.interpret_output_py(&prototype.output_ty)?;

    todo!()
  }

  fn interpret_pattern(&mut self, _pattern: &ast::Pattern) -> Result<Value> {
    todo!()
  }

  fn interpret_inputs(&mut self, _inputs: &ast::Inputs) -> Result<Value> {
    todo!()
  }

  fn interpret_output_py(
    &mut self,
    _output_py: &ast::OutputTy,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_block(&mut self, block: &ast::Block) -> Result<Value> {
    let mut value = Value::new(0usize);

    for stmt in &block.stmts {
      value = self.interpret_stmt(stmt)?;
    }

    Ok(value)
  }

  fn interpret_stmt(&mut self, stmt: &ast::Stmt) -> Result<Value> {
    match &stmt.kind {
      ast::StmtKind::Expr(expr) => self.interpret_stmt_expr(expr),
      _ => todo!(),
    }
  }

  fn interpret_stmt_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    self.interpret_expr(expr)
  }

  fn interpret_expr(&mut self, expr: &ast::Expr) -> Result<Value> {
    match &expr.kind {
      ast::ExprKind::Lit(lit) => self.interpret_expr_lit(lit),
      ast::ExprKind::UnOp(unop, rhs) => self.interpret_expr_unop(unop, rhs),
      ast::ExprKind::BinOp(binop, lhs, rhs) => {
        self.interpret_expr_binop(binop, lhs, rhs)
      }
      _ => todo!(),
    }
  }

  fn interpret_expr_lit(&mut self, lit: &ast::Lit) -> Result<Value> {
    match &lit.kind {
      ast::LitKind::Int(symbol) => self.interpret_expr_lit_int(symbol),
      ast::LitKind::Float(symbol) => self.interpret_expr_lit_float(symbol),
      ast::LitKind::Ident(symbol) => self.interpret_expr_lit_ident(symbol),
      ast::LitKind::Bool(boolean) => self.interpret_expr_lit_boolean(boolean),
      ast::LitKind::Char(symbol) => self.interpret_expr_lit_char(symbol),
      ast::LitKind::Str(symbol) => self.interpret_expr_lit_str(symbol),
    }
  }

  fn interpret_expr_lit_int(&mut self, symbol: &Symbol) -> Result<Value> {
    let _int = self.interner.lookup_int(*symbol);

    // Ok(self.builder.ins().iconst(types::I64, int))
    todo!()
  }

  fn interpret_expr_lit_float(&mut self, symbol: &Symbol) -> Result<Value> {
    let _float = self.interner.lookup_float(*symbol);

    // Ok(self.builder.ins().f64const(float))
    todo!()
  }

  fn interpret_expr_lit_ident(&mut self, symbol: &Symbol) -> Result<Value> {
    let _ident = self.interner.lookup_ident(*symbol);

    todo!()
  }

  fn interpret_expr_lit_boolean(&mut self, _boolean: &bool) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_lit_char(&mut self, symbol: &Symbol) -> Result<Value> {
    let _ch = self.interner.lookup_char(*symbol);

    todo!()
  }

  fn interpret_expr_lit_str(&mut self, symbol: &Symbol) -> Result<Value> {
    let _string = self.interner.lookup_str(*symbol);

    todo!()
  }

  fn interpret_expr_unop(
    &mut self,
    _unop: &ast::UnOp,
    _rhs: &ast::Expr,
  ) -> Result<Value> {
    todo!()
  }

  fn interpret_expr_binop(
    &mut self,
    _binop: &ast::BinOp,
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
) -> Result<*const u8> {
  Interpreter::new(&mut session.interner)?.interpret(program)
}

pub struct Translator<'program> {
  builder: FunctionBuilder<'program>,
  module: &'program JITModule,
}

impl<'program> Translator<'program> {
  pub fn new(
    builder: FunctionBuilder<'program>,
    module: &'program JITModule,
  ) -> Self {
    Self { builder, module }
  }

  fn translate(&mut self, _program: &ast::Program) -> Result<()> {
    todo!()
  }
}
