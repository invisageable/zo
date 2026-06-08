//! In-page reactivity for the web bundle.
//!
//! The webview drives reactivity from the Rust host (IPC →
//! `HandlerEvaluator` → DOM patch). A static `public/` bundle has no
//! host, so the same loop is emitted as **in-page JS**: the `mut`
//! state, the event handlers (zo closures transpiled to JS), and the
//! **compile-time binding graph** — a write fires only the bindings its
//! slot drives. No signals, no whole-stream diff, no re-render.

use zo_interner::{Interner, Symbol};
use zo_sir::{BinOp, Insn, LoadSource, Sir, UnOp};
use zo_ty::Mutability;
use zo_ui_protocol::{EventKind, UiCommand};
use zo_value::FunctionKind;

/// Emits the in-page reactive runtime for a `#render` program.
pub struct ReactiveJs<'a> {
  sir: &'a Sir,
  interner: &'a Interner,
}

impl<'a> ReactiveJs<'a> {
  /// A reactive-JS emitter over `sir`.
  pub fn new(sir: &'a Sir, interner: &'a Interner) -> Self {
    Self { sir, interner }
  }

  /// Emit the page's reactive `<script>` body, or `None` for a static
  /// page (no click handlers). `text_bindings` is the rebased
  /// `(cmd_idx, var)` table; `commands` carries the `Event` wiring.
  pub fn emit(
    &self,
    text_bindings: &[(usize, Symbol)],
    commands: &[UiCommand],
  ) -> Option<String> {
    let mut handlers: Vec<(String, String)> = Vec::new();
    let mut state_vars: Vec<Symbol> = Vec::new();

    // The candidate state set is the program's `mut` vars — a handler
    // `Load`/`Store` of one reads/writes `state[…]`. Per-closure capture
    // lists are unreliable: a second closure over the same var records
    // none. The `state` object itself holds only the vars handlers
    // actually write, so template-internal counters never leak in.
    let mut_vars = self.mut_vars();

    for cmd in commands {
      let UiCommand::Event {
        widget_id,
        event_kind: EventKind::Click,
        handler,
      } = cmd
      else {
        continue;
      };

      if handler.is_empty() {
        continue;
      }

      if let Some((body, written)) = self.transpile_handler(handler, &mut_vars)
      {
        handlers.push((widget_id.clone(), body));

        for var in written {
          if !state_vars.contains(&var) {
            state_vars.push(var);
          }
        }
      }
    }

    if handlers.is_empty() {
      return None;
    }

    let state_init = self.state_init(&state_vars);
    let bindings = self.binding_map(text_bindings);
    let handlers = handlers
      .iter()
      .map(|(id, body)| format!("{}:function(e){{{body}}}", js_str(id)))
      .collect::<Vec<_>>()
      .join(",");

    Some(format!(
      "(function(){{\
       var state={{{state_init}}};\
       var bindings={{{bindings}}};\
       function fire(slot){{var cmds=bindings[slot];if(!cmds)return;\
       for(var i=0;i<cmds.length;i++){{\
       var el=document.querySelector('[data-zo-cmd=\"'+cmds[i]+'\"]');\
       if(el)el.textContent=state[slot];}}}}\
       var handlers={{{handlers}}};\
       document.addEventListener('click',function(e){{\
       var b=e.target.closest&&e.target.closest('button[data-id]');\
       if(b&&handlers[b.dataset.id])handlers[b.dataset.id](e);}});\
       }})();"
    ))
  }

  /// `"var":initial, …` for the state object.
  fn state_init(&self, vars: &[Symbol]) -> String {
    vars
      .iter()
      .map(|v| {
        format!("{}:{}", js_str(self.interner.get(*v)), self.initial(*v))
      })
      .collect::<Vec<_>>()
      .join(",")
  }

  /// `"var":[cmd, …], …` — the compile-time binding graph: each state
  /// var maps to the text commands it drives.
  fn binding_map(&self, text_bindings: &[(usize, Symbol)]) -> String {
    let mut map: Vec<(Symbol, Vec<usize>)> = Vec::new();

    for (cmd_idx, var) in text_bindings {
      match map.iter_mut().find(|(v, _)| v == var) {
        Some((_, cmds)) => cmds.push(*cmd_idx),
        None => map.push((*var, vec![*cmd_idx])),
      }
    }

    map
      .iter()
      .map(|(var, cmds)| {
        let list = cmds
          .iter()
          .map(usize::to_string)
          .collect::<Vec<_>>()
          .join(",");

        format!("{}:[{list}]", js_str(self.interner.get(*var)))
      })
      .collect::<Vec<_>>()
      .join(",")
  }

  /// The program's `mut` vars — these are the reactive state slots.
  fn mut_vars(&self) -> Vec<Symbol> {
    self
      .sir
      .instructions
      .iter()
      .filter_map(|insn| match insn {
        Insn::VarDef {
          name,
          mutability: Mutability::Yes,
          ..
        } => Some(*name),
        _ => None,
      })
      .collect()
  }

  /// Transpile one closure handler to a JS statement sequence plus the
  /// state vars it writes (to seed `state` and fire afterward). `None`
  /// when the body uses an `Insn` this slice doesn't lower yet (control
  /// flow, the event payload, list mutation) — the handler is then
  /// skipped. `mut_vars` are the reactive candidates: a `Load`/`Store`
  /// of one reads/writes `state[…]`; any other local bails the handler.
  fn transpile_handler(
    &self,
    handler: &str,
    mut_vars: &[Symbol],
  ) -> Option<(String, Vec<Symbol>)> {
    let mut start = None;
    let mut end = self.sir.instructions.len();

    for (i, insn) in self.sir.instructions.iter().enumerate() {
      if let Insn::FunDef {
        name,
        kind: FunctionKind::Closure { .. },
        ..
      } = insn
        && self.interner.get(*name) == handler
      {
        start = Some(i + 1);

        for (j, next) in self.sir.instructions.iter().enumerate().skip(i + 1) {
          if matches!(next, Insn::FunDef { .. }) {
            end = j;
            break;
          }
        }

        break;
      }
    }

    let start = start?;
    let is_state = |sym: &Symbol| mut_vars.contains(sym);

    let mut js = String::new();
    let mut dirty: Vec<Symbol> = Vec::new();

    for insn in &self.sir.instructions[start..end] {
      match insn {
        Insn::ConstInt { dst, value, .. } => {
          js.push_str(&format!("var v{}={};", dst.0, *value as i64));
        }
        Insn::ConstFloat { dst, value, .. } => {
          js.push_str(&format!("var v{}={value};", dst.0));
        }
        Insn::ConstBool { dst, value, .. } => {
          js.push_str(&format!("var v{}={value};", dst.0));
        }
        Insn::ConstString { dst, symbol, .. } => {
          js.push_str(&format!(
            "var v{}={};",
            dst.0,
            js_str(self.interner.get(*symbol)),
          ));
        }
        Insn::Load {
          dst,
          src: LoadSource::Local(sym),
          ..
        } if is_state(sym) => {
          js.push_str(&format!(
            "var v{}=state[{}];",
            dst.0,
            js_str(self.interner.get(*sym)),
          ));
        }
        Insn::BinOp {
          dst, op, lhs, rhs, ..
        } => {
          js.push_str(&format!(
            "var v{}=v{}{}v{};",
            dst.0,
            lhs.0,
            binop_js(op),
            rhs.0,
          ));
        }
        Insn::UnOp { dst, op, rhs, .. } => {
          js.push_str(&format!("var v{}={}v{};", dst.0, unop_js(op), rhs.0));
        }
        Insn::Store { name, value, .. } if is_state(name) => {
          js.push_str(&format!(
            "state[{}]=v{};",
            js_str(self.interner.get(*name)),
            value.0,
          ));

          if !dirty.contains(name) {
            dirty.push(*name);
          }
        }
        // The closure body ends at its `Return`. The body range can
        // run to EOF for the last closure (no following `FunDef`), so
        // stop here rather than walk into the template instructions.
        Insn::Return { .. } => break,
        // Control flow, event payload, list mutation, non-state locals
        // — not lowered yet; skip the whole handler so no broken JS
        // reaches the page.
        _ => return None,
      }
    }

    for var in &dirty {
      js.push_str(&format!("fire({});", js_str(self.interner.get(*var))));
    }

    Some((js, dirty))
  }

  /// The JS literal for a state var's initial value.
  fn initial(&self, var: Symbol) -> String {
    for insn in &self.sir.instructions {
      let Insn::VarDef {
        name,
        init: Some(init),
        ..
      } = insn
      else {
        continue;
      };

      if *name != var {
        continue;
      }

      for prev in &self.sir.instructions {
        match prev {
          Insn::ConstInt { dst, value, .. } if dst == init => {
            return format!("{}", *value as i64);
          }
          Insn::ConstFloat { dst, value, .. } if dst == init => {
            return format!("{value}");
          }
          Insn::ConstBool { dst, value, .. } if dst == init => {
            return format!("{value}");
          }
          Insn::ConstString { dst, symbol, .. } if dst == init => {
            return js_str(self.interner.get(*symbol));
          }
          _ => {}
        }
      }

      return "0".to_string();
    }

    "0".to_string()
  }
}

/// The JS operator for a zo binary op.
fn binop_js(op: &BinOp) -> &'static str {
  match op {
    BinOp::Add | BinOp::Concat => "+",
    BinOp::Sub => "-",
    BinOp::Mul => "*",
    BinOp::Div => "/",
    BinOp::Rem => "%",
    BinOp::Eq => "===",
    BinOp::Neq => "!==",
    BinOp::Lt => "<",
    BinOp::Lte => "<=",
    BinOp::Gt => ">",
    BinOp::Gte => ">=",
    BinOp::And => "&&",
    BinOp::Or => "||",
    BinOp::BitAnd => "&",
    BinOp::BitOr => "|",
    BinOp::BitXor => "^",
    BinOp::Shl => "<<",
    BinOp::Shr => ">>",
  }
}

/// The JS operator for a zo unary op.
fn unop_js(op: &UnOp) -> &'static str {
  match op {
    UnOp::Neg => "-",
    UnOp::Not => "!",
    UnOp::BitNot => "~",
  }
}

/// Escape `s` as a JS string literal (incl. `<`/`>` to keep it safe
/// inside an inline `<script>`).
fn js_str(s: &str) -> String {
  let mut out = String::with_capacity(s.len() + 2);

  out.push('"');

  for c in s.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '<' => out.push_str("\\x3c"),
      '>' => out.push_str("\\x3e"),
      _ => out.push(c),
    }
  }

  out.push('"');
  out
}
