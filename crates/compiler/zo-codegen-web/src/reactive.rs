//! In-page reactivity for the web bundle.
//!
//! The webview drives reactivity from the Rust host (IPC →
//! `HandlerEvaluator` → DOM patch). A static `public/` bundle has no
//! host, so the same loop is emitted as **in-page JS**: the `mut`
//! state, the event handlers (zo closures transpiled to JS), and the
//! **compile-time binding graph** — a write fires only the bindings its
//! slot drives. No signals, no whole-stream diff, no re-render.

use crate::transpile::{FnTranspiler, js_name};

use zo_interner::{Interner, Symbol};
use zo_sir::{
  BinOp, Insn, ListItemCmd, LoadSource, Sir, TemplateBindings, UnOp,
};
use zo_ty::Mutability;
use zo_ui_protocol::{Attr, EventKind, PropValue, UiCommand};
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
  /// page (no click handlers). `bindings` is the rebased binding graph;
  /// `commands` carries the `Event` wiring.
  ///
  /// Reactivity is fine-grained: a write to a state var fires only the
  /// DOM patch ops that var drives — text content, element attributes,
  /// computed text, and list re-renders.
  pub fn emit(
    &self,
    bindings: &TemplateBindings,
    commands: &[UiCommand],
  ) -> Option<String> {
    let mut handlers: Vec<(String, String)> = Vec::new();
    let mut state_vars: Vec<Symbol> = Vec::new();

    // The candidate state set is the program's `mut` vars — a handler
    // `Load`/`Store` of one reads/writes `state[…]`. Per-closure capture
    // lists are unreliable: a second closure over the same var records
    // none.
    let mut_vars = self.mut_vars();
    // User functions the handlers call — they ship as JS alongside.
    let mut handler_roots: Vec<Symbol> = Vec::new();

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
          push_unique(&mut state_vars, var);
        }

        if let Some((start, end, _)) = self.closure_body(handler) {
          for insn in &self.sir.instructions[start..end] {
            if let Insn::Call { name, .. } = insn {
              push_unique(&mut handler_roots, *name);
            }
          }
        }
      }
    }

    if handlers.is_empty() {
      return None;
    }

    // Transpile every user function the handlers and reactive
    // initialisers reach (`glyph`, `card_of`, the `Rng` methods) to JS.
    // `None` → a reachable call has no transpilable body (an FFI): fall
    // back to a static page rather than emit a call to a function that
    // doesn't exist in the browser.
    let transpiler = FnTranspiler::new(self.sir, self.interner);
    let main_range = transpiler.main_range();
    let mut roots = handler_roots;

    if let Some(range) = main_range {
      for sym in transpiler.init_roots(range) {
        push_unique(&mut roots, sym);
      }
    }

    let functions = transpiler.emit_reachable(&roots)?;

    // The compile-time binding graph: each reactive var → the DOM patch
    // ops its writes drive. Op encodings: `["t",cmd]` text content,
    // `["a",cmd,name]` attribute, `["c",cmd]` computed text, `["l",cmd]`
    // list re-render.
    let mut ops: Vec<(Symbol, Vec<String>)> = Vec::new();
    // Computed slots: `cmd → JS function body returning the value`.
    let mut computed: Vec<(usize, String)> = Vec::new();
    // List slots: `cmd → JS function body returning the items' HTML`.
    let mut lists: Vec<(usize, String)> = Vec::new();
    // Array-typed state vars (a list's `items_var`) — they seed to `[]`.
    let mut array_vars: Vec<Symbol> = Vec::new();

    for (cmd_idx, var) in &bindings.text {
      push_unique(&mut state_vars, *var);
      add_op(&mut ops, *var, format!("[\"t\",{cmd_idx}]"));
    }

    for (cmd_idx, attr) in &bindings.attrs {
      let Attr::Dynamic { name, var, .. } = attr else {
        continue;
      };

      let var = Symbol(*var);

      push_unique(&mut state_vars, var);
      add_op(&mut ops, var, format!("[\"a\",{cmd_idx},{}]", js_str(name)));
    }

    for (cmd_idx, binding) in &bindings.computed {
      let closure = self.interner.get(binding.closure_name);

      let Some(body) =
        self.transpile_value(closure, &binding.captures, &mut_vars)
      else {
        continue;
      };

      // Any captured var re-runs this computed slot when it changes.
      for var in &binding.captures {
        push_unique(&mut state_vars, *var);
        add_op(&mut ops, *var, format!("[\"c\",{cmd_idx}]"));
      }

      computed.push((*cmd_idx, body));
    }

    for (cmd_idx, binding) in &bindings.list {
      let items_var = binding.items_var;

      push_unique(&mut state_vars, items_var);
      push_unique(&mut array_vars, items_var);
      add_op(&mut ops, items_var, format!("[\"l\",{cmd_idx}]"));
      lists.push((
        *cmd_idx,
        self.list_render(items_var, &binding.item_template),
      ));
    }

    let state_init =
      self.state_init(&state_vars, &array_vars, &transpiler, main_range);
    let binds = self.binding_object(&ops);
    let computed = fn_table(&computed);
    let lists = fn_table(&lists);
    let handlers = handlers
      .iter()
      .map(|(id, body)| format!("{}:function(e){{{body}}}", js_str(id)))
      .collect::<Vec<_>>()
      .join(",");

    Some(format!(
      "(function(){{\
       {functions}\
       var state={{{state_init}}};\
       var binds={{{binds}}};\
       var computed={{{computed}}};\
       var lists={{{lists}}};\
       function esc(s){{return String(s).replace(/[&<\\x3e\"]/g,function(c){{\
       return{{\"&\":\"&amp;\",\"<\":\"&lt;\",\"\\x3e\":\"&gt;\",'\"':\"&quot;\"}}[c];}});}}\
       function q(c){{return document.querySelector('[data-zo-cmd=\"'+c+'\"]');}}\
       function fire(slot){{var ops=binds[slot];if(!ops)return;\
       for(var i=0;i<ops.length;i++){{var op=ops[i],el=q(op[1]);if(!el)continue;\
       if(op[0]===\"t\")el.textContent=state[slot];\
       else if(op[0]===\"a\"){{var an=op[2];\
       if(an===\"checked\"||an===\"disabled\"||an===\"selected\")\
       el[an]=state[slot]&&state[slot]!==\"false\";\
       else el.setAttribute(an,state[slot]);}}\
       else if(op[0]===\"c\")el.textContent=computed[op[1]]();\
       else if(op[0]===\"l\")el.innerHTML=lists[op[1]]();}}}}\
       var handlers={{{handlers}}};\
       document.addEventListener('click',function(e){{\
       var b=e.target.closest&&e.target.closest('button[data-id]');\
       if(b&&handlers[b.dataset.id])handlers[b.dataset.id](e);}});\
       for(var s in binds)fire(s);\
       }})();"
    ))
  }

  /// `"var":initial, …` for the state object. A var in `array_vars`
  /// (a list's backing `[]T`) seeds to an empty array; the rest take
  /// their scalar `VarDef` initial.
  fn state_init(
    &self,
    vars: &[Symbol],
    array_vars: &[Symbol],
    transpiler: &FnTranspiler,
    main_range: Option<(usize, usize)>,
  ) -> String {
    vars
      .iter()
      .map(|v| {
        let init = if array_vars.contains(v) {
          "[]".to_string()
        } else {
          self.initial(*v, transpiler, main_range)
        };

        format!("{}:{init}", js_str(self.interner.get(*v)))
      })
      .collect::<Vec<_>>()
      .join(",")
  }

  /// A list slot's JS body: map the backing array to per-item HTML and
  /// join it. The placeholder element's `innerHTML` is set to the
  /// result on every `fire(items_var)`.
  fn list_render(&self, items_var: Symbol, template: &[ListItemCmd]) -> String {
    format!(
      "return state[{}].map(function(t){{return {};}}).join(\"\");",
      js_str(self.interner.get(items_var)),
      self.list_item_expr(template),
    )
  }

  /// The JS string expression that renders one list item. Static tags
  /// and literal text are baked in; `TextFromItem` becomes the
  /// HTML-escaped item value (`esc(t)`).
  fn list_item_expr(&self, template: &[ListItemCmd]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut open_tags: Vec<&str> = Vec::new();

    for step in template {
      match step {
        ListItemCmd::Element { tag, attrs } => {
          let tag = tag.as_str();
          let mut open = format!("<{tag}");

          for attr in attrs {
            if let Attr::Prop { name, value } = attr
              && let PropValue::Str(value) = value
            {
              open.push_str(&format!(" {name}=\"{value}\""));
            }
          }

          open.push('>');
          parts.push(js_str(&open));
          open_tags.push(tag);
        }
        ListItemCmd::EndElement => {
          if let Some(tag) = open_tags.pop() {
            parts.push(js_str(&format!("</{tag}>")));
          }
        }
        ListItemCmd::Text(text) => parts.push(js_str(text)),
        ListItemCmd::TextFromItem => parts.push("esc(t)".to_string()),
      }
    }

    if parts.is_empty() {
      "\"\"".to_string()
    } else {
      parts.join("+")
    }
  }

  /// `"var":[op, …], …` — the binding graph as a JS object: each state
  /// var maps to the DOM patch ops its writes drive.
  fn binding_object(&self, ops: &[(Symbol, Vec<String>)]) -> String {
    ops
      .iter()
      .map(|(var, var_ops)| {
        format!(
          "{}:[{}]",
          js_str(self.interner.get(*var)),
          var_ops.join(",")
        )
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
    let (start, end, params) = self.closure_body(handler)?;
    let is_state = |sym: &Symbol| mut_vars.contains(sym);

    let mut js = String::new();
    let mut dirty: Vec<Symbol> = Vec::new();
    // State vars the handler reads (not necessarily writes) — they must
    // be seeded so `state[…]` exists, e.g. a captured `rng` the body
    // threads through `Rng::range`.
    let mut reads: Vec<Symbol> = Vec::new();

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

          if !reads.contains(sym) {
            reads.push(*sym);
          }
        }
        // A captured reactive var reads through its `Param` slot —
        // e.g. an array a handler mutates before its list binding is
        // declared captures, so the body loads `param[i]` rather than
        // a free `Local`. Map it to the state cell; a non-state param
        // (the event payload) isn't lowered yet, so bail.
        Insn::Load {
          dst,
          src: LoadSource::Param(index),
          ..
        } => {
          let sym = params.get(*index as usize).filter(|s| is_state(s))?;

          js.push_str(&format!(
            "var v{}=state[{}];",
            dst.0,
            js_str(self.interner.get(*sym)),
          ));

          if !reads.contains(sym) {
            reads.push(*sym);
          }
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
        // `arr.push(x)` on a reactive `[]T`. `owner` names the array
        // var, so we mutate `state[arr]` directly (a JS array is a
        // reference) and fire its list re-render.
        Insn::ArrayPush {
          value,
          owner: Some(owner),
          ..
        } => {
          js.push_str(&format!(
            "state[{}].push(v{});",
            js_str(self.interner.get(*owner)),
            value.0,
          ));

          if !dirty.contains(owner) {
            dirty.push(*owner);
          }
        }
        // A call to a user function — `glyph`, `card_of`, a `Rng`
        // method. The callee ships as JS via the function transpiler
        // (reachability is gathered alongside in `emit`), so the handler
        // just invokes it by its `js_name`.
        Insn::Call {
          dst, name, args, ..
        } => {
          let passed: Vec<String> =
            args.iter().map(|v| format!("v{}", v.0)).collect();

          js.push_str(&format!(
            "var v{}={}({});",
            dst.0,
            js_name(self.interner, *name),
            passed.join(","),
          ));
        }
        // The closure body ends at its `Return`. The body range can
        // run to EOF for the last closure (no following `FunDef`), so
        // stop here rather than walk into the template instructions.
        Insn::Return { .. } => break,
        // Control flow, event payload, an array push with no owning var,
        // non-state locals — not lowered yet; skip the whole handler so
        // no broken JS reaches the page.
        _ => return None,
      }
    }

    for var in &dirty {
      js.push_str(&format!("fire({});", js_str(self.interner.get(*var))));
    }

    // Seed every state var the handler touches — writes (which also
    // fire) and reads alike.
    let mut seed = dirty;

    for sym in reads {
      if !seed.contains(&sym) {
        seed.push(sym);
      }
    }

    Some((js, seed))
  }

  /// A closure's body range `[start, end)` plus its param symbols in
  /// `Param`-index order (captures first, then user params). The body is
  /// the instructions after the `FunDef`, up to the next `FunDef` (or
  /// EOF for the last closure). `None` when no such closure exists.
  fn closure_body(&self, name: &str) -> Option<(usize, usize, Vec<Symbol>)> {
    let mut found = None;
    let mut end = self.sir.instructions.len();

    for (i, insn) in self.sir.instructions.iter().enumerate() {
      if let Insn::FunDef {
        name: fun_name,
        params,
        kind: FunctionKind::Closure { .. },
        ..
      } = insn
        && self.interner.get(*fun_name) == name
      {
        let symbols = params.iter().map(|(sym, _)| *sym).collect();

        found = Some((i + 1, symbols));

        for (j, next) in self.sir.instructions.iter().enumerate().skip(i + 1) {
          if matches!(next, Insn::FunDef { .. }) {
            end = j;
            break;
          }
        }

        break;
      }
    }

    found.map(|(start, symbols)| (start, end, symbols))
  }

  /// Transpile a computed-binding closure to a JS function body that
  /// `return`s the expression's value. Its captures are read by param
  /// index against `state` (`Param(i)` → `captures[i]`). `None` when the
  /// body uses an `Insn` this slice doesn't lower yet.
  fn transpile_value(
    &self,
    closure: &str,
    captures: &[Symbol],
    mut_vars: &[Symbol],
  ) -> Option<String> {
    let (start, end, _params) = self.closure_body(closure)?;

    let mut js = String::new();

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
          src: LoadSource::Param(index),
          ..
        } => {
          let sym = captures.get(*index as usize)?;

          js.push_str(&format!(
            "var v{}=state[{}];",
            dst.0,
            js_str(self.interner.get(*sym)),
          ));
        }
        Insn::Load {
          dst,
          src: LoadSource::Local(sym),
          ..
        } if mut_vars.contains(sym) => {
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
        Insn::Return {
          value: Some(value), ..
        } => {
          js.push_str(&format!("return v{};", value.0));

          return Some(js);
        }
        // A computed binding must yield a value; anything else (a void
        // return, control flow, an unlowered op) means we can't emit it.
        _ => return None,
      }
    }

    None
  }

  /// A state var's initial value as a JS expression. Resolved within
  /// `main`'s range (where the reactive `VarDef`s live), through the
  /// function transpiler so a non-literal initialiser — `face =
  /// glyph(Card::Three)` — becomes a real call (`f$..glyph([2])`), not
  /// the `0` an int-only scan would leave. Falls back to `0` when the
  /// initialiser can't be lowered.
  fn initial(
    &self,
    var: Symbol,
    transpiler: &FnTranspiler,
    main_range: Option<(usize, usize)>,
  ) -> String {
    let Some(range) = main_range else {
      return "0".to_string();
    };

    for insn in &self.sir.instructions[range.0..range.1] {
      if let Insn::VarDef {
        name,
        init: Some(init),
        ..
      } = insn
        && *name == var
      {
        return transpiler
          .init_expr(init.0, range)
          .unwrap_or_else(|| "0".to_string());
      }
    }

    "0".to_string()
  }
}

/// `cmd:function(){body}, …` — a JS object literal mapping each command
/// index to a zero-arg function with the given body. Shared by the
/// computed-text and list-render slot tables.
fn fn_table(slots: &[(usize, String)]) -> String {
  slots
    .iter()
    .map(|(cmd_idx, body)| format!("{cmd_idx}:function(){{{body}}}"))
    .collect::<Vec<_>>()
    .join(",")
}

/// Push `sym` to `vars` if not already present.
fn push_unique(vars: &mut Vec<Symbol>, sym: Symbol) {
  if !vars.contains(&sym) {
    vars.push(sym);
  }
}

/// Append `op` (a JS array literal) to `var`'s op list in `ops`,
/// creating the entry on first use. Preserves insertion order so the
/// emitted graph is deterministic.
fn add_op(ops: &mut Vec<(Symbol, Vec<String>)>, var: Symbol, op: String) {
  match ops.iter_mut().find(|(v, _)| *v == var) {
    Some((_, list)) => list.push(op),
    None => ops.push((var, vec![op])),
  }
}

/// The JS operator for a zo binary op.
pub(crate) fn binop_js(op: &BinOp) -> &'static str {
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
pub(crate) fn unop_js(op: &UnOp) -> &'static str {
  match op {
    UnOp::Neg => "-",
    UnOp::Not => "!",
    UnOp::BitNot => "~",
  }
}

/// Escape `s` as a JS string literal (incl. `<`/`>` to keep it safe
/// inside an inline `<script>`).
pub(crate) fn js_str(s: &str) -> String {
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

#[cfg(test)]
mod tests {
  use super::ReactiveJs;

  use zo_interner::Interner;
  use zo_sir::{
    BinOp, Insn, ListBinding, ListItemCmd, LoadSource, Sir, TemplateBindings,
  };
  use zo_span::Span;
  use zo_ty::{Mutability, SelfKind, TyId};
  use zo_ui_protocol::{Attr, ElementTag, EventKind, PropValue, UiCommand};
  use zo_value::{FunctionKind, Pubness, ValueId};

  const TY: TyId = TyId(0);

  /// An `__closure_0` `FunDef` with no params or captures.
  fn closure(interner: &mut Interner) -> Insn {
    Insn::FunDef {
      name: interner.intern("__closure_0"),
      params: Vec::new(),
      return_ty: TY,
      body_start: 0,
      kind: FunctionKind::Closure { capture_count: 0 },
      pubness: Pubness::No,
      self_kind: SelfKind::None,
      link_name: None,
      owning_pack: None,
      span: Span::ZERO,
      is_test: false,
    }
  }

  /// A `Click` event wired to `handler` on widget `"0"`.
  fn click(handler: &str) -> UiCommand {
    UiCommand::Event {
      widget_id: "0".to_string(),
      event_kind: EventKind::Click,
      handler: handler.to_string(),
    }
  }

  #[test]
  fn text_and_attr_bindings_share_one_state_var() {
    let mut interner = Interner::new();
    let count = interner.intern("count");
    let mut sir = Sir::new();

    // `mut count = 0;`
    sir.emit(Insn::ConstInt {
      dst: ValueId(0),
      value: 0,
      ty_id: TY,
    });
    sir.emit(Insn::VarDef {
      name: count,
      ty_id: TY,
      init: Some(ValueId(0)),
      mutability: Mutability::Yes,
      pubness: Pubness::No,
    });

    // `fn() => count += 1`
    sir.emit(closure(&mut interner));
    sir.emit(Insn::Load {
      dst: ValueId(1),
      src: LoadSource::Local(count),
      ty_id: TY,
    });
    sir.emit(Insn::ConstInt {
      dst: ValueId(2),
      value: 1,
      ty_id: TY,
    });
    sir.emit(Insn::BinOp {
      dst: ValueId(3),
      op: BinOp::Add,
      lhs: ValueId(1),
      rhs: ValueId(2),
      ty_id: TY,
    });
    sir.emit(Insn::Store {
      name: count,
      value: ValueId(3),
      ty_id: TY,
    });
    sir.emit(Insn::Return {
      value: None,
      ty_id: TY,
    });

    let commands = vec![click("__closure_0")];
    let bindings = TemplateBindings {
      text: vec![(4, count)],
      attrs: vec![(
        3,
        Attr::Dynamic {
          name: "title".to_string(),
          var: count.0,
          initial: PropValue::Str("0".to_string()),
        },
      )],
      computed: Vec::new(),
      list: Vec::new(),
      conditional: Vec::new(),
    };

    let js = ReactiveJs::new(&sir, &interner)
      .emit(&bindings, &commands)
      .expect("a click handler emits a runtime");

    assert!(js.contains("var state={\"count\":0}"), "{js}");
    // One var driving both a text and an attribute op.
    assert!(
      js.contains("\"count\":[[\"t\",4],[\"a\",3,\"title\"]]"),
      "{js}"
    );
    // Non-boolean attrs still setAttribute (via the `an` alias);
    // boolean attrs (checked/disabled/selected) set the property.
    assert!(js.contains("el.setAttribute(an,state[slot])"));
    assert!(js.contains("el[an]=state[slot]&&state[slot]!==\"false\""));
    // The handler writes state and fires the var.
    assert!(js.contains("state[\"count\"]=v3;fire(\"count\");"), "{js}");
  }

  #[test]
  fn list_binding_seeds_array_and_renders_items() {
    let mut interner = Interner::new();
    let items = interner.intern("items");
    let hi = interner.intern("hi");
    let mut sir = Sir::new();

    // `mut items = [];`
    sir.emit(Insn::ArrayLiteral {
      dst: ValueId(0),
      elements: Vec::new(),
      ty_id: TY,
    });
    sir.emit(Insn::VarDef {
      name: items,
      ty_id: TY,
      init: Some(ValueId(0)),
      mutability: Mutability::Yes,
      pubness: Pubness::No,
    });

    // `fn() => items.push("hi")`
    sir.emit(closure(&mut interner));
    sir.emit(Insn::ConstString {
      dst: ValueId(1),
      symbol: hi,
      ty_id: TY,
    });
    sir.emit(Insn::ArrayPush {
      array: ValueId(0),
      value: ValueId(1),
      ty_id: TY,
      owner: Some(items),
    });
    sir.emit(Insn::Return {
      value: None,
      ty_id: TY,
    });

    let commands = vec![click("__closure_0")];
    let bindings = TemplateBindings {
      text: Vec::new(),
      attrs: Vec::new(),
      computed: Vec::new(),
      list: vec![(
        5,
        ListBinding {
          items_var: items,
          item_template: vec![
            ListItemCmd::Element {
              tag: ElementTag::Li,
              attrs: Vec::new(),
            },
            ListItemCmd::TextFromItem,
            ListItemCmd::EndElement,
          ],
        },
      )],
      conditional: Vec::new(),
    };

    let js = ReactiveJs::new(&sir, &interner)
      .emit(&bindings, &commands)
      .expect("a list program with a click handler emits a runtime");

    // The backing array seeds empty and drives a list re-render op.
    assert!(js.contains("var state={\"items\":[]}"), "{js}");
    assert!(js.contains("\"items\":[[\"l\",5]]"), "{js}");
    // The list slot maps the array to escaped `<li>` items.
    assert!(js.contains("state[\"items\"].map(function(t)"), "{js}");
    assert!(js.contains("esc(t)"));
    // The handler pushes by owner and fires the array.
    assert!(
      js.contains("state[\"items\"].push(v1);fire(\"items\");"),
      "{js}"
    );
    // Computed and list slots get an initial paint on load, so a
    // non-empty first frame renders before any event fires.
    assert!(js.contains("for(var l in lists)"), "{js}");
  }
}
