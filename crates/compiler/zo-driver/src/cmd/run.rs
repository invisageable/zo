use crate::args;
use crate::cmd::Handle;
use crate::constants::EXIT_CODE_ERROR;

use zo_bundler::ios::simulator::Simulator;
use zo_compiler::{Analyzed, Compiler};
use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_runtime::{Runtime, Server};
use zo_runtime_render::reactive::{
  ResolvedComputedBindings, apply_computed_bindings, apply_list_bindings,
};
use zo_runtime_render::render::{EventRegistry, Graphics, RuntimeConfig};
use zo_runtime_render::render::{StateCell, StateValue};
use zo_sir::{ComputedBinding, Insn, ListBinding, ListItemCmd};
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
  /// List bindings: `(cmd_idx, ListBinding)`. Each entry
  /// targets a placeholder `UiCommand::Text(_)` slot that
  /// the runtime replaces with the rendered list items
  /// (one item per element of `items_var`'s state cell)
  /// on every reactive update.
  list_bindings: &'a [(usize, ListBinding)],
  commands: &'a [UiCommand],
  shared_cmds: std::sync::Arc<std::sync::Mutex<Vec<UiCommand>>>,
}

#[derive(clap::Args, Debug)]
pub(crate) struct Run {
  #[command(flatten)]
  pub(crate) args: args::Args,
  /// The iOS Simulator device for `run --target ios`.
  #[arg(long, default_value = "iPhone 17 Pro")]
  pub(crate) device: String,
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
    compiler.configure_diagnostics(zo_compiler::DiagnosticsConfig {
      format: self.args.format.into(),
      snippet_context: self.args.snippet_context,
      explain_decisions: self.args.explain_decisions,
    });

    let (semantic, tokenization, parsing, session, file_table) =
      compiler.analyze_source(&source, input_path);

    // iOS and web both build an artifact and hand off to a Runtimer —
    // the Simulator for iOS, a local server + browser for web. Neither
    // uses the in-process command/event runtime below, so they branch
    // out here on the shared analysis.
    if self.args.target.is_ios() || self.args.target.is_web() {
      let analyzed = Analyzed {
        semantic,
        tokenization,
        parsing,
        session,
        file_table,
      };

      return if self.args.target.is_web() {
        self.run_web(&mut compiler, &analyzed)
      } else {
        self.run_ios(&mut compiler, &analyzed, input_path)
      };
    }

    // Collect the set of template ValueIds targeted by `#render`
    // directives. Templates are component definitions —
    // rendering every `Insn::Template` indiscriminately would
    // also render intermediate components that are only meant
    // to be inlined into a parent via `<nested />`.
    let mut dom_targets: Vec<ValueId> = Vec::new();
    let mut has_dom_directive = false;

    for insn in &semantic.sir.instructions {
      if let Insn::Directive { name, value, .. } = insn
        && zo_ui_protocol::is_render_directive(session.interner.get(*name))
      {
        has_dom_directive = true;
        dom_targets.push(*value);
      }
    }

    // Extract UI commands and bindings only from templates
    // that are actually targeted by a `#render` directive.
    let mut ui_commands = Vec::new();
    let mut text_bindings: Vec<(usize, Symbol)> = Vec::new();
    let mut attr_bindings: Vec<(usize, zo_ui_protocol::Attr)> = Vec::new();
    let mut computed_bindings: Vec<(usize, ComputedBinding)> = Vec::new();
    // List bindings flow through the same offset-rebase as
    // their siblings so per-template index spaces compose
    // without collisions. Reactive expansion (re-rendering
    // when the bound `mut []T` changes) lands behind the
    // runtime's array-StateCell support — until that arrives,
    // the placeholder text emitted by the executor renders
    // as an empty slot, which is correct for the empty-array
    // initial frame.
    let mut list_bindings: Vec<(usize, ListBinding)> = Vec::new();

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

        for (cmd_idx, lb) in &bindings.list {
          list_bindings.push((base + cmd_idx, lb.clone()));
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
      let graphics = if self.args.target.is_webview() {
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
        || !computed_bindings.is_empty()
        || !list_bindings.is_empty();
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
          list_bindings: &list_bindings,
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
      // emits `LC_LOAD_DYLIB @loader_path/deps/libzo_runtime
      // .dylib` and the compiler stages the dylib into a
      // sibling `deps/` directory. A flat shared `temp_dir()`
      // setup means every run overwrites the same dylib path
      // while earlier runs (or zombie dyld-stuck processes)
      // may still hold the file open — which on macOS leaves
      // the new run wedged in `dyld3::MachOFile::compatible
      // Slice` indefinitely. A fresh subdirectory per run
      // sidesteps the whole class.
      let run_dir =
        std::env::temp_dir().join(format!("zo_run_{}", std::process::id()));

      let _ = std::fs::create_dir_all(&run_dir);

      let temp_path = run_dir.join("a.out");

      // Reuse the single analysis from above — codegen and
      // link the program we already analyzed for template
      // detection, instead of re-analyzing it through
      // `compile`. One analysis per program, one diagnostic
      // report.
      let analyzed = Analyzed {
        semantic,
        tokenization,
        parsing,
        session,
        file_table,
      };

      compiler.compile_analyzed(
        &analyzed,
        self.args.target.into(),
        &temp_path,
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

  /// Build the iOS `.app` from the already-analyzed program and hand
  /// it to the Simulator: boot, install, launch.
  fn run_ios(
    &self,
    compiler: &mut Compiler,
    analyzed: &Analyzed,
    input_path: &std::path::Path,
  ) -> Result<(), Error> {
    let Some(name) = input_path.file_stem().and_then(|s| s.to_str()) else {
      eprintln!(
        "Error: cannot derive an app name from {}",
        input_path.display(),
      );

      std::process::exit(EXIT_CODE_ERROR);
    };

    // Build the `.app` into a per-run directory so its path is stable
    // and isolated from concurrent runs.
    let out_dir =
      std::env::temp_dir().join(format!("zo_run_ios_{}", std::process::id()));

    let _ = std::fs::create_dir_all(&out_dir);

    let output_path = out_dir.join(name);

    compiler.compile_analyzed(
      analyzed,
      self.args.target.into(),
      &output_path,
    )?;

    let app = output_path.with_extension("app");
    let bundle_id = zo_bundler::bundle_id(name);
    let simulator = Simulator::new(&self.device);

    if let Err(error) = simulator.launch(&app, &bundle_id) {
      eprintln!("Error launching iOS Simulator: {error}");

      std::process::exit(EXIT_CODE_ERROR);
    }

    Ok(())
  }

  /// Build the `public/` bundle from this analysis, then serve it on
  /// localhost and open the browser. Blocks until the process is
  /// killed (Ctrl-C), like the in-process window paths.
  fn run_web(
    &self,
    compiler: &mut Compiler,
    analyzed: &Analyzed,
  ) -> Result<(), Error> {
    // Build into a per-run `public/` dir so its path is stable and
    // isolated from concurrent runs.
    let public = std::env::temp_dir()
      .join(format!("zo_run_web_{}", std::process::id()))
      .join("public");

    let _ = std::fs::create_dir_all(&public);

    compiler.compile_analyzed(analyzed, self.args.target.into(), &public)?;

    if let Err(error) = Server::new(&public).serve() {
      eprintln!("Error serving web bundle: {error}");

      std::process::exit(EXIT_CODE_ERROR);
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
            Box::new(move |payload| {
              println!("[zo] event handler '{handler_name}' called");
              println!("[zo] event payload '{payload}' passed");
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

    // List-binding `items_var`s back the per-event list
    // re-render. The cell carries `StateValue::Strs(...)`
    // initialized from the SIR's `ArrayLiteral` (empty for
    // `mut todos: []str = []`).
    for (_cmd_idx, lb) in ctx.list_bindings {
      register_slot(lb.items_var, &mut state_slots);
    }

    // Every reactive var keyed to its cell slot, so a handler that
    // mutates a template mutable without capturing it (the `+` of a
    // counter — the executor leaves a post-binding closure's mutable
    // free) still writes the right cell.
    let state_syms: Vec<(Symbol, usize)> = state_slots
      .iter()
      .enumerate()
      .map(|(slot_idx, (sym, _, _))| (*sym, slot_idx))
      .collect();

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

      let computed_binds =
        Self::resolve_computed_bindings(computed_bindings, &state_slots);

      let list_binds: Vec<(usize, usize, Vec<ListItemCmd>)> = ctx
        .list_bindings
        .iter()
        .filter_map(|(cmd_idx, lb)| {
          state_slots
            .iter()
            .position(|(s, _, _)| *s == lb.items_var)
            .map(|slot_idx| (*cmd_idx, slot_idx, lb.item_template.clone()))
        })
        .collect();

      let commands_copy = commands.to_vec();
      let shared = shared_cmds.clone();
      let sir = sir_arc.clone();
      let strings = strings_arc.clone();
      let closure_sym = *name;
      let computed_binds_clone = computed_binds.clone();
      let list_binds_clone = list_binds.clone();
      let state_syms_clone = state_syms.clone();

      registry.register(
        fun_name,
        Box::new(move |payload| {
          let mut eval = zo_runtime_render::evaluator::HandlerEvaluator::new()
            .with_state_syms(state_syms_clone.clone());

          eval.execute(
            &sir,
            closure_sym,
            &cells,
            &capture_map,
            &strings,
            Some(payload),
          );

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

          apply_computed_bindings(
            &mut new_cmds,
            &computed_binds_clone,
            &cells,
            &sir,
            &strings,
          );

          // List re-render runs LAST — `splice` shifts
          // tail indices, but no other binding sits past
          // a list anchor in the wip's shape. When more
          // complex layouts appear, the binding-index
          // remap belongs in the executor's
          // `optimize_with_indices`-style pass.
          apply_list_bindings(&mut new_cmds, &list_binds_clone, &cells);

          *shared.lock().unwrap() = new_cmds;
        }),
      );
    }

    // Initial render: invoke each computed/list binding
    // once before the runtime starts so the first frame
    // shows the correct content instead of the empty
    // placeholders the executor pushed.
    if !computed_bindings.is_empty() || !ctx.list_bindings.is_empty() {
      let mut new_cmds = commands.to_vec();
      let cells: Vec<StateCell> = state_slots
        .iter()
        .map(|(_, _, cell)| cell.clone())
        .collect();

      let computed_binds =
        Self::resolve_computed_bindings(computed_bindings, &state_slots);

      apply_computed_bindings(
        &mut new_cmds,
        &computed_binds,
        &cells,
        &sir_arc,
        &strings_arc,
      );

      let list_binds: Vec<(usize, usize, Vec<ListItemCmd>)> = ctx
        .list_bindings
        .iter()
        .filter_map(|(cmd_idx, lb)| {
          state_slots
            .iter()
            .position(|(s, _, _)| *s == lb.items_var)
            .map(|slot_idx| (*cmd_idx, slot_idx, lb.item_template.clone()))
        })
        .collect();

      apply_list_bindings(&mut new_cmds, &list_binds, &cells);

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
  ) -> ResolvedComputedBindings {
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
              Insn::ArrayLiteral { dst, elements, .. } if dst == init_id => {
                // `mut items: []str = ["a", "b"]` —
                // resolve each element through its
                // ConstString to seed the initial array
                // contents. Non-string element types fall
                // through to the empty `Strs` default.
                let mut items: Vec<String> = Vec::with_capacity(elements.len());

                for elem_id in elements {
                  let resolved = instructions.iter().find_map(|i| match i {
                    Insn::ConstString { dst, symbol, .. } if dst == elem_id => {
                      Some(interner.get(*symbol).to_string())
                    }
                    _ => None,
                  });

                  if let Some(s) = resolved {
                    items.push(s);
                  }
                }

                return StateValue::Strs(items);
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
