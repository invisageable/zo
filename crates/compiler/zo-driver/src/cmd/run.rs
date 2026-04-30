use crate::args;
use crate::cmd::Handle;
use crate::constants::EXIT_CODE_ERROR;

use zo_compiler::Compiler;
use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_runtime::Runtime;
use zo_runtime_render::render::{EventRegistry, Graphics, RuntimeConfig};
use zo_runtime_render::render::{StateCell, StateValue};
use zo_sir::{ComputedBinding, Insn};
use zo_span::Span;
use zo_ui_protocol::{Ui, UiCommand};
use zo_value::FunctionKind;
use zo_value::ValueId;

/// Parameters for building reactive event handlers.
struct ReactiveContext<'a> {
  instructions: &'a [Insn],
  interner: &'a zo_interner::Interner,
  handler_names: &'a [String],
  /// Reactive bindings that target `UiCommand::Text(_)`
  /// content at the given command index.
  text_bindings: &'a [(usize, Symbol)],
  /// Reactive bindings that target a named attribute on a
  /// `UiCommand::Element` at the given command index. The
  /// `Attr` entries are always `Attr::Dynamic` — the runtime
  /// calls `UiCommand::set_attr` to apply each patch.
  attr_bindings: &'a [(usize, zo_ui_protocol::Attr)],
  /// Computed text bindings: `(cmd_idx, ComputedBinding)`
  /// pointing at a `UiCommand::Text(_)` whose value is
  /// recomputed by invoking the binding's closure over the
  /// captured locals on every reactive update.
  computed_bindings: &'a [(usize, ComputedBinding)],
  commands: &'a [UiCommand],
  shared_cmds: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
}

#[derive(clap::Args, Debug)]
pub(crate) struct Run {
  #[command(flatten)]
  pub(crate) args: args::Args,
}

impl Run {
  fn run(&self) -> Result<(), Error> {
    if self.args.files.is_empty() {
      eprintln!("Error: No input files specified");
      std::process::exit(EXIT_CODE_ERROR);
    }

    let input_path = &self.args.files[0];
    let source = crate::cmd::read_source(input_path);
    let search_paths = crate::cmd::search_paths(input_path);

    let mut compiler = Compiler::with_search_paths(search_paths);

    let (semantic, _tokenization, _parsing, session) =
      compiler.analyze_source(&source, input_path);

    // Collect the set of template ValueIds targeted by `#dom`
    // directives. Templates are component definitions —
    // rendering every `Insn::Template` indiscriminately would
    // also render intermediate components that are only meant
    // to be inlined into a parent via `<nested />`.
    let mut dom_targets: Vec<ValueId> = Vec::new();
    let mut has_dom_directive = false;

    for insn in &semantic.sir.instructions {
      if let Insn::Directive { name, value, .. } = insn
        && session.interner.get(*name) == "dom"
      {
        has_dom_directive = true;
        dom_targets.push(*value);
      }
    }

    // Extract UI commands and bindings only from templates
    // that are actually targeted by a `#dom` directive.
    let mut ui_commands = Vec::new();
    let mut text_bindings: Vec<(usize, Symbol)> = Vec::new();
    let mut attr_bindings: Vec<(usize, zo_ui_protocol::Attr)> = Vec::new();
    let mut computed_bindings: Vec<(usize, ComputedBinding)> = Vec::new();

    for insn in &semantic.sir.instructions {
      if let Insn::Template {
        id,
        commands,
        bindings,
        ..
      } = insn
        && dom_targets.contains(id)
      {
        let base = ui_commands.len();

        ui_commands.extend_from_slice(commands);

        for (cmd_idx, sym) in &bindings.text {
          text_bindings.push((base + cmd_idx, *sym));
        }

        for (cmd_idx, attr) in &bindings.attrs {
          attr_bindings.push((base + cmd_idx, attr.clone()));
        }

        for (cmd_idx, cb) in &bindings.computed {
          computed_bindings.push((base + cmd_idx, cb.clone()));
        }
      }
    }

    // Resolve relative image paths to absolute paths using
    // the source file's parent directory as the base. Both
    // native and web runtimes need absolute paths — native
    // for filesystem reads, web for the `zo://localhost`
    // custom protocol. Remote URLs pass through untouched.
    let mut ui = Ui::new(ui_commands);

    if let Some(base_dir) = input_path.parent() {
      ui.resolve_image_paths(base_dir);
    }

    let ui_commands = ui.into_commands();

    // Template path: launch runtime.
    if has_dom_directive && !ui_commands.is_empty() {
      let graphics = if self.args.web {
        Graphics::Web
      } else {
        Graphics::Native
      };

      println!(
        "Running template with {} UI commands \
         ({graphics:?} mode)...",
        ui_commands.len(),
      );

      // Collect handler names from Event commands.
      let mut handler_names: Vec<String> = Vec::new();

      for cmd in &ui_commands {
        if let UiCommand::Event { handler, .. } = cmd
          && !handler.is_empty()
          && !handler_names.contains(handler)
        {
          handler_names.push(handler.clone());
        }
      }

      // Detect reactive template. A template with only
      // computed bindings (no `mut`-driven simple text or
      // attr bindings) still needs the reactive path so the
      // computed closures fire on each event.
      let has_bindings = !text_bindings.is_empty()
        || !attr_bindings.is_empty()
        || !computed_bindings.is_empty();
      let is_reactive = has_bindings
        && handler_names.iter().any(|h| h.starts_with("__closure_"));

      let config = RuntimeConfig {
        library_path: None,
        title: format!(
          "{}",
          input_path.file_name().unwrap_or_default().to_string_lossy()
        ),
        size: (800.0, 600.0),
        graphics,
      };

      let mut runtime = Runtime::with_config(config);

      runtime.set_commands(ui_commands.clone());

      // Get shared command buffer for reactive handlers.
      let shared_cmds = runtime.shared_commands();

      let mut event_registry = EventRegistry::new();

      if is_reactive {
        let ctx = ReactiveContext {
          instructions: &semantic.sir.instructions,
          interner: &session.interner,
          handler_names: &handler_names,
          text_bindings: &text_bindings,
          attr_bindings: &attr_bindings,
          computed_bindings: &computed_bindings,
          commands: &ui_commands,
          shared_cmds,
        };

        self.build_reactive_handlers(&ctx, &mut event_registry);
      } else {
        self.build_static_handlers(
          &semantic.sir.instructions,
          &session.interner,
          &handler_names,
          &mut event_registry,
        );
      }

      runtime.set_events(event_registry);

      runtime.run().map_err(|_| {
        Error::new(ErrorKind::InternalCompilerError, Span::ZERO)
      })?;
    } else {
      // Programming path: compile to a per-run isolated
      // dir, execute. The isolation matters: the codegen
      // emits `LC_LOAD_DYLIB @executable_path/libzo_runtime
      // .dylib` and the compiler stages the dylib next to
      // the binary. A flat shared `temp_dir()` setup means
      // every run overwrites the same dylib path while
      // earlier runs (or zombie dyld-stuck processes) may
      // still hold the file open — which on macOS leaves
      // the new run wedged in `dyld3::MachOFile::compatible
      // Slice` indefinitely. A fresh subdirectory per run
      // sidesteps the whole class.
      let run_dir =
        std::env::temp_dir().join(format!("zo_run_{}", std::process::id()));

      let _ = std::fs::create_dir_all(&run_dir);

      let temp_path = run_dir.join("a.out");

      compiler.compile(
        &[(input_path, source.clone())],
        self.args.target.into(),
        &[],
        &Some(temp_path.clone()),
      )?;

      // Run with cwd set to the source file's parent so
      // relative paths in `read_file(...)` etc. resolve
      // against the program's own directory regardless of
      // where `zo run` was invoked from. Mirrors zo-test-
      // runner's behaviour and keeps `zo run path/to.zo`
      // identical from any shell cwd.
      let mut cmd = std::process::Command::new(&temp_path);

      cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

      if let Some(parent) = input_path.parent()
        && !parent.as_os_str().is_empty()
      {
        cmd.current_dir(parent);
      }

      let status = cmd.status();

      let _ = std::fs::remove_dir_all(&run_dir);

      match status {
        Ok(s) if !s.success() => {
          std::process::exit(s.code().unwrap_or(EXIT_CODE_ERROR));
        }
        Err(e) => {
          eprintln!("Error executing program: {e}");
          std::process::exit(EXIT_CODE_ERROR);
        }
        _ => {}
      }
    }

    Ok(())
  }

  /// Register static event handlers (non-reactive path).
  fn build_static_handlers(
    &self,
    instructions: &[Insn],
    interner: &zo_interner::Interner,
    handler_names: &[String],
    registry: &mut EventRegistry,
  ) {
    for insn in instructions {
      if let Insn::FunDef { name, .. } = insn {
        let fun_name = interner.get(*name).to_string();

        if handler_names.contains(&fun_name) {
          let handler_name = fun_name.clone();

          registry.register(
            fun_name,
            Box::new(move || {
              println!("[zo] event handler '{handler_name}' called");
            }),
          );
        }
      }
    }
  }

  /// Build reactive event handlers with shared state.
  fn build_reactive_handlers(
    &self,
    ctx: &ReactiveContext<'_>,
    registry: &mut EventRegistry,
  ) {
    let instructions = ctx.instructions;
    let interner = ctx.interner;
    let handler_names = ctx.handler_names;
    let text_bindings = ctx.text_bindings;
    let attr_bindings = ctx.attr_bindings;
    let computed_bindings = ctx.computed_bindings;
    let commands = ctx.commands;
    let shared_cmds = &ctx.shared_cmds;
    // Shared SIR instructions for all handler closures
    // (avoids cloning per handler).
    let sir_arc = std::sync::Arc::new(instructions.to_vec());
    // Per-symbol string snapshot for the evaluator's
    // `Insn::ConstString` resolution. Cloned by `Arc` into
    // each handler closure — `Interner` itself isn't
    // `Clone`-friendly across threads.
    let strings_arc = std::sync::Arc::new(interner.snapshot());

    // Create state cells for each bound variable. Both text
    // and attribute bindings can reference the same variable,
    // so we dedupe by symbol.
    let mut state_slots: Vec<(Symbol, String, StateCell)> = Vec::new();

    let register_slot =
      |sym: Symbol, slots: &mut Vec<(Symbol, String, StateCell)>| {
        if slots.iter().any(|(s, _, _)| *s == sym) {
          return;
        }

        let var_name = interner.get(sym).to_string();
        let initial = Self::find_initial_value(instructions, sym, interner);

        slots.push((sym, var_name, StateCell::new(initial)));
      };

    for (_cmd_idx, sym) in text_bindings {
      register_slot(*sym, &mut state_slots);
    }

    for (_cmd_idx, attr) in attr_bindings {
      if let zo_ui_protocol::Attr::Dynamic { var, .. } = attr {
        // `Attr::Dynamic.var` carries the raw interner id
        // (u32) so ui-protocol doesn't depend on zo-interner.
        let sym = Symbol(*var);

        register_slot(sym, &mut state_slots);
      }
    }

    // Computed-binding captures drive their closures; each
    // capture needs a state cell so the evaluator can
    // resolve `Load Local(sym)` against shared state.
    for (_cmd_idx, cb) in computed_bindings {
      for sym in &cb.captures {
        register_slot(*sym, &mut state_slots);
      }
    }

    // Register closure handlers.
    for insn in instructions {
      let (name, capture_count, params) = match insn {
        Insn::FunDef {
          name,
          kind: FunctionKind::Closure { capture_count },
          params,
          ..
        } => (name, *capture_count, params),
        _ => continue,
      };

      let fun_name = interner.get(*name).to_string();

      if !handler_names.contains(&fun_name) {
        continue;
      }

      // Build capture map: param index → state slot index.
      let cc = capture_count as usize;
      let mut capture_map: Vec<(usize, usize)> = Vec::new();

      for (pi, &(cap_name, _)) in params.iter().enumerate().take(cc) {
        if let Some(slot_idx) =
          state_slots.iter().position(|(s, _, _)| *s == cap_name)
        {
          capture_map.push((pi, slot_idx));
        }
      }

      // Clone data for the handler closure.
      let cells: Vec<StateCell> = state_slots
        .iter()
        .map(|(_, _, cell)| cell.clone())
        .collect();

      // Text bindings — (cmd_idx, slot_idx) pairs.
      let text_binds: Vec<(usize, usize)> = text_bindings
        .iter()
        .filter_map(|(cmd_idx, sym)| {
          state_slots
            .iter()
            .position(|(s, _, _)| s == sym)
            .map(|slot_idx| (*cmd_idx, slot_idx))
        })
        .collect();

      // Attribute bindings — (cmd_idx, attr_name, slot_idx)
      // triples. `attr_name` is pre-extracted from the Dynamic
      // attr so the patch closure doesn't need to re-match the
      // enum on every invocation.
      let attr_binds: Vec<(usize, String, usize)> = attr_bindings
        .iter()
        .filter_map(|(cmd_idx, attr)| {
          if let zo_ui_protocol::Attr::Dynamic { name, var, .. } = attr {
            let sym = Symbol(*var);

            state_slots
              .iter()
              .position(|(s, _, _)| *s == sym)
              .map(|slot_idx| (*cmd_idx, name.clone(), slot_idx))
          } else {
            None
          }
        })
        .collect();

      let computed_binds = Self::resolve_computed_bindings(
        computed_bindings,
        &state_slots,
      );

      let commands_copy = commands.to_vec();
      let shared = shared_cmds.clone();
      let sir = sir_arc.clone();
      let strings = strings_arc.clone();
      let closure_sym = *name;
      let computed_binds_clone = computed_binds.clone();

      registry.register(
        fun_name,
        Box::new(move || {
          let mut eval = zo_runtime_render::evaluator::HandlerEvaluator::new();

          eval.execute(&sir, closure_sym, &cells, &capture_map, &strings);

          let mut new_cmds = commands_copy.clone();

          for &(cmd_idx, slot_idx) in &text_binds {
            let value = cells[slot_idx].get().display();

            if let Some(UiCommand::Text(s)) = new_cmds.get_mut(cmd_idx) {
              *s = value;
            }
          }

          // Element attribute patches dispatch through
          // `UiCommand::set_attr` which handles the per-variant
          // field updates uniformly.
          for (cmd_idx, attr_name, slot_idx) in &attr_binds {
            let value = cells[*slot_idx].get().display();

            if let Some(cmd) = new_cmds.get_mut(*cmd_idx) {
              cmd.set_attr(attr_name, &value);
            }
          }

          Self::apply_computed_bindings(
            &mut new_cmds,
            &computed_binds_clone,
            &cells,
            &sir,
            &strings,
          );

          *shared.lock().unwrap() = new_cmds;
        }),
      );
    }

    // Initial render: invoke each computed binding once
    // before the runtime starts so the first frame shows
    // the correct text instead of the empty placeholder
    // the executor pushed. Shares the per-event helper.
    if !computed_bindings.is_empty() {
      let mut new_cmds = commands.to_vec();
      let cells: Vec<StateCell> = state_slots
        .iter()
        .map(|(_, _, cell)| cell.clone())
        .collect();

      let computed_binds =
        Self::resolve_computed_bindings(computed_bindings, &state_slots);

      Self::apply_computed_bindings(
        &mut new_cmds,
        &computed_binds,
        &cells,
        &sir_arc,
        &strings_arc,
      );

      *shared_cmds.lock().unwrap() = new_cmds;
    }
  }

  /// Resolve each `ComputedBinding`'s capture list against
  /// the state-slot table into the `(cmd_idx, closure_name,
  /// capture_map)` triple shape `apply_computed_bindings`
  /// consumes — same shape as click handlers' capture_map.
  fn resolve_computed_bindings(
    computed_bindings: &[(usize, ComputedBinding)],
    state_slots: &[(Symbol, String, StateCell)],
  ) -> Vec<(usize, Symbol, Vec<(usize, usize)>)> {
    computed_bindings
      .iter()
      .map(|(cmd_idx, cb)| {
        let cap_map = cb
          .captures
          .iter()
          .enumerate()
          .filter_map(|(pi, sym)| {
            state_slots
              .iter()
              .position(|(s, _, _)| s == sym)
              .map(|slot_idx| (pi, slot_idx))
          })
          .collect();

        (*cmd_idx, cb.closure_name, cap_map)
      })
      .collect()
  }

  /// Re-run each computed binding's closure over the
  /// current state cells and stamp the returned `Val`
  /// (rendered via `display()`) into its bound
  /// `UiCommand::Text` slot. Shared by the per-event patch
  /// loop and the initial-render pass — both paths must
  /// drive the same evaluator + same string snapshot or
  /// they'll drift.
  fn apply_computed_bindings(
    new_cmds: &mut [UiCommand],
    computed_binds: &[(usize, Symbol, Vec<(usize, usize)>)],
    cells: &[StateCell],
    sir: &[Insn],
    strings: &[String],
  ) {
    for (cmd_idx, closure_name, cap_map) in computed_binds {
      let mut eval = zo_runtime_render::evaluator::HandlerEvaluator::new();

      let result =
        eval.execute(sir, *closure_name, cells, cap_map, strings);

      if let Some(val) = result
        && let Some(UiCommand::Text(s)) = new_cmds.get_mut(*cmd_idx)
      {
        *s = val.display();
      }
    }
  }

  /// Find the initial value of a variable from VarDef in SIR.
  fn find_initial_value(
    instructions: &[Insn],
    var_sym: Symbol,
    interner: &zo_interner::Interner,
  ) -> StateValue {
    for insn in instructions {
      if let Insn::VarDef { name, init, .. } = insn
        && *name == var_sym
      {
        if let Some(init_id) = init {
          for prev in instructions {
            match prev {
              Insn::ConstInt { dst, value, .. } if dst == init_id => {
                return StateValue::Int(*value as i64);
              }
              Insn::ConstFloat { dst, value, .. } if dst == init_id => {
                return StateValue::Float(*value);
              }
              Insn::ConstBool { dst, value, .. } if dst == init_id => {
                return StateValue::Bool(*value);
              }
              Insn::ConstString { dst, symbol, .. } if dst == init_id => {
                let s = interner.get(*symbol).to_string();

                return StateValue::Str(s);
              }
              _ => {}
            }
          }
        }

        return StateValue::Int(0);
      }
    }

    StateValue::Int(0)
  }
}

impl Handle for Run {
  fn handle(&self) {
    crate::cmd::handle_with_watch(&self.args, || self.run());
  }
}
