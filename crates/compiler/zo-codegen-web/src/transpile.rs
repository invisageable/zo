//! General zo→JS transpiler for the user functions a web program's
//! reactive handlers and initial values reach. The browser has no host
//! runtime, so a handler like `face = glyph(card_of(rng.range(…)))` must
//! ship `glyph`, `card_of`, and the `Rng` methods as JS too — not just
//! the closure itself.
//!
//! Lowered SIR is unstructured (labels + branches), so each function
//! body becomes a label-dispatch loop (`for(;;)switch(pc){…}`): a `jmp`
//! or taken branch sets `pc` and re-enters the switch, sequential blocks
//! fall through. Every aggregate — struct, enum, tuple — is a JS array;
//! `field .N` is `[N]` and an enum's tag is index 0. `ValueId`s are
//! per-function, so each body is read only within its own range.

use super::reactive::{binop_js, js_str, unop_js};

use zo_interner::{Interner, Symbol};
use zo_sir::{Insn, LoadSource, Sir};
use zo_value::FunctionKind;

use std::collections::{HashMap, HashSet};

/// A transpilable function body: param count and the `[start, end)`
/// instruction range (after the `FunDef` marker, up to the next one).
struct Body {
  params: usize,
  start: usize,
  end: usize,
}

/// Transpiles the user functions reachable from a web program's
/// reactive entry points (handlers + initial values) into JS.
pub(crate) struct FnTranspiler<'a> {
  sir: &'a Sir,
  interner: &'a Interner,
  defs: HashMap<Symbol, Body>,
}

impl<'a> FnTranspiler<'a> {
  /// Index every transpilable `FunDef` (skips `Intrinsic` — those are
  /// FFI with no zo body) by name.
  pub(crate) fn new(sir: &'a Sir, interner: &'a Interner) -> Self {
    let insns = &sir.instructions;
    let mut defs = HashMap::new();

    for (i, insn) in insns.iter().enumerate() {
      let Insn::FunDef {
        name, params, kind, ..
      } = insn
      else {
        continue;
      };

      if matches!(kind, FunctionKind::Intrinsic) {
        continue;
      }

      defs.insert(
        *name,
        Body {
          params: params.len(),
          start: i + 1,
          end: next_fundef(insns, i + 1),
        },
      );
    }

    Self {
      sir,
      interner,
      defs,
    }
  }

  /// JS definitions for every function transitively reachable from
  /// `roots`. `None` when a reachable call has no transpilable body (an
  /// FFI/intrinsic) — the caller then falls back to a static page.
  pub(crate) fn emit_reachable(&self, roots: &[Symbol]) -> Option<String> {
    let mut seen: HashSet<Symbol> = HashSet::new();
    let mut queue: Vec<Symbol> = roots.to_vec();
    let mut out = String::new();

    while let Some(sym) = queue.pop() {
      if !seen.insert(sym) {
        continue;
      }

      let body = self.defs.get(&sym)?;

      out.push_str(&self.transpile_fn(sym, body)?);

      for insn in &self.sir.instructions[body.start..body.end] {
        if let Insn::Call { name, .. } = insn {
          queue.push(*name);
        }
      }
    }

    Some(out)
  }

  /// The call targets a reactive var's initialiser reaches (so they
  /// seed the reachable set). Only transpilable defs are returned; an
  /// FFI call in an initialiser is skipped here and surfaces later as a
  /// failed `init_expr`.
  pub(crate) fn init_roots(&self, range: (usize, usize)) -> Vec<Symbol> {
    self.sir.instructions[range.0..range.1]
      .iter()
      .filter_map(|insn| match insn {
        Insn::Call { name, .. } if self.defs.contains_key(name) => Some(*name),
        _ => None,
      })
      .collect()
  }

  /// One function as `function NAME(a0,…){ … label-dispatch … }`.
  fn transpile_fn(&self, sym: Symbol, body: &Body) -> Option<String> {
    let insns = &self.sir.instructions[body.start..body.end];

    // Locals: every symbol a store/def writes or a local load reads.
    // Declared up front so a write in one block is visible in another.
    let mut locals: Vec<u32> = Vec::new();

    for insn in insns {
      let local = match insn {
        Insn::Store { name, .. }
        | Insn::VarDef { name, .. }
        | Insn::ConstDef { name, .. } => name.0,
        Insn::Load {
          src: LoadSource::Local(s),
          ..
        } => s.0,
        _ => continue,
      };

      if !locals.contains(&local) {
        locals.push(local);
      }
    }

    // Label id → block index; the entry block (no label) is 0.
    let mut block_of: HashMap<u32, usize> = HashMap::new();
    let mut block = 0usize;

    for insn in insns {
      if let Insn::Label { id } = insn {
        block += 1;
        block_of.insert(*id, block);
      }
    }

    let args: Vec<String> = (0..body.params).map(|i| format!("a{i}")).collect();
    let mut out =
      format!("function {}({}){{", js_name(self.interner, sym), args.join(","));

    if !locals.is_empty() {
      let decls: Vec<String> = locals.iter().map(|n| format!("l{n}")).collect();
      out.push_str(&format!("var {};", decls.join(",")));
    }

    out.push_str("var pc=0;for(;;){switch(pc){case 0:");

    // `dead` skips instructions after an unconditional terminator
    // (`jmp`/`ret`) until the next label opens a fresh block.
    let mut dead = false;

    for insn in insns {
      match insn {
        Insn::Label { id } => {
          dead = false;
          out.push_str(&format!("case {}:", block_of.get(id)?));
        }
        _ if dead => {}
        Insn::Jump { target } => {
          out.push_str(&format!("pc={};break;", block_of.get(target)?));
          dead = true;
        }
        Insn::BranchIfNot { cond, target } => {
          out.push_str(&format!(
            "if(!v{}){{pc={};break;}}",
            cond.0,
            block_of.get(target)?,
          ));
        }
        Insn::Return { value, .. } => {
          match value {
            Some(v) => out.push_str(&format!("return v{};", v.0)),
            None => out.push_str("return;"),
          }
          dead = true;
        }
        other => out.push_str(&self.stmt(other)?),
      }
    }

    out.push_str("default:return;}}}");

    Some(out)
  }

  /// One straight-line instruction as a JS statement. `None` for an
  /// instruction the web backend can't lower (bails the whole bundle).
  fn stmt(&self, insn: &Insn) -> Option<String> {
    Some(match insn {
      Insn::ConstInt { dst, value, .. } => {
        format!("var v{}={};", dst.0, *value as i64)
      }
      Insn::ConstFloat { dst, value, .. } => format!("var v{}={value};", dst.0),
      Insn::ConstBool { dst, value, .. } => format!("var v{}={value};", dst.0),
      Insn::ConstString { dst, symbol, .. } => {
        format!("var v{}={};", dst.0, js_str(self.interner.get(*symbol)))
      }
      Insn::Load {
        dst,
        src: LoadSource::Param(i),
        ..
      } => format!("var v{}=a{i};", dst.0),
      Insn::Load {
        dst,
        src: LoadSource::Local(s),
        ..
      } => format!("var v{}=l{};", dst.0, s.0),
      Insn::Store { name, value, .. } => format!("l{}=v{};", name.0, value.0),
      Insn::TupleIndex {
        dst, tuple, index, ..
      } => format!("var v{}=v{}[{index}];", dst.0, tuple.0),
      Insn::FieldStore {
        base, index, value, ..
      } => format!("v{}[{index}]=v{};", base.0, value.0),
      Insn::EnumConstruct {
        dst, variant, fields, ..
      } => {
        let mut parts = vec![variant.to_string()];
        parts.extend(fields.iter().map(|f| format!("v{}", f.0)));
        format!("var v{}=[{}];", dst.0, parts.join(","))
      }
      Insn::StructConstruct { dst, fields, .. } => {
        let parts: Vec<String> =
          fields.iter().map(|f| format!("v{}", f.0)).collect();
        format!("var v{}=[{}];", dst.0, parts.join(","))
      }
      Insn::TupleLiteral { dst, elements, .. } => {
        let parts: Vec<String> =
          elements.iter().map(|e| format!("v{}", e.0)).collect();
        format!("var v{}=[{}];", dst.0, parts.join(","))
      }
      Insn::BinOp {
        dst, op, lhs, rhs, ..
      } => format!("var v{}=(v{}{}v{});", dst.0, lhs.0, binop_js(op), rhs.0),
      Insn::UnOp { dst, op, rhs, .. } => {
        format!("var v{}=({}v{});", dst.0, unop_js(op), rhs.0)
      }
      Insn::Call {
        dst, name, args, ..
      } => {
        let passed: Vec<String> =
          args.iter().map(|v| format!("v{}", v.0)).collect();

        format!(
          "var v{}={}({});",
          dst.0,
          js_name(self.interner, *name),
          passed.join(","),
        )
      }
      // Declarations are no-ops (the matching `Store` assigns); scope
      // drops and nops carry no runtime effect.
      Insn::VarDef { .. }
      | Insn::ConstDef { .. }
      | Insn::Drop { .. }
      | Insn::Nop => String::new(),
      _ => return None,
    })
  }

  /// A reactive var's initial value as a JS expression, resolved within
  /// `range` (`main`'s body — `ValueId`s are per-function). Handles
  /// literals, enum/struct construction, and calls, recursively:
  /// `face = glyph(Card::Three)` becomes `f$..glyph([2])`. `None` when
  /// the chain hits an instruction it can't lower.
  pub(crate) fn init_expr(&self, vid: u32, range: (usize, usize)) -> Option<String> {
    for insn in &self.sir.instructions[range.0..range.1] {
      match insn {
        Insn::ConstInt { dst, value, .. } if dst.0 == vid => {
          return Some(format!("{}", *value as i64));
        }
        Insn::ConstFloat { dst, value, .. } if dst.0 == vid => {
          return Some(format!("{value}"));
        }
        Insn::ConstBool { dst, value, .. } if dst.0 == vid => {
          return Some(format!("{value}"));
        }
        Insn::ConstString { dst, symbol, .. } if dst.0 == vid => {
          return Some(js_str(self.interner.get(*symbol)));
        }
        Insn::EnumConstruct {
          dst, variant, fields, ..
        } if dst.0 == vid => {
          let mut parts = vec![variant.to_string()];

          for f in fields {
            parts.push(self.init_expr(f.0, range)?);
          }

          return Some(format!("[{}]", parts.join(",")));
        }
        Insn::StructConstruct { dst, fields, .. } if dst.0 == vid => {
          let mut parts = Vec::new();

          for f in fields {
            parts.push(self.init_expr(f.0, range)?);
          }

          return Some(format!("[{}]", parts.join(",")));
        }
        Insn::Call {
          dst, name, args, ..
        } if dst.0 == vid => {
          let mut passed = Vec::new();

          for arg in args {
            passed.push(self.init_expr(arg.0, range)?);
          }

          return Some(format!(
            "{}({})",
            js_name(self.interner, *name),
            passed.join(","),
          ));
        }
        _ => {}
      }
    }

    None
  }

  /// The `[start, end)` range of `main`'s body, where the reactive
  /// `VarDef`s and their initialisers live.
  pub(crate) fn main_range(&self) -> Option<(usize, usize)> {
    let insns = &self.sir.instructions;

    for (i, insn) in insns.iter().enumerate() {
      if let Insn::FunDef { name, .. } = insn {
        let name = self.interner.get(*name);

        if name == "main" || name.ends_with("::main") {
          return Some((i + 1, next_fundef(insns, i + 1)));
        }
      }
    }

    None
  }
}

/// The index of the next `FunDef` at or after `from`, or the end of the
/// stream — the exclusive upper bound of a function's body.
fn next_fundef(insns: &[Insn], from: usize) -> usize {
  insns[from..]
    .iter()
    .position(|insn| matches!(insn, Insn::FunDef { .. }))
    .map_or(insns.len(), |offset| from + offset)
}

/// A JS-safe function name for a zo symbol: `f$` + the symbol text with
/// every non-identifier byte replaced by `_`. The `f$` prefix avoids a
/// digit-leading name (`001_cards::glyph`) and collisions with the
/// reactive runtime's own identifiers.
pub(crate) fn js_name(interner: &Interner, sym: Symbol) -> String {
  let mut out = String::from("f$");

  for ch in interner.get(sym).chars() {
    if ch.is_ascii_alphanumeric() || ch == '_' {
      out.push(ch);
    } else {
      out.push('_');
    }
  }

  out
}
