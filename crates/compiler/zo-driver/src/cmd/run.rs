use crate::args;
use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use zo_compiler::Compiler;
use zo_error::{Error, ErrorKind};
use zo_interner::Symbol;
use zo_runtime::{EventRegistry, Graphics, Runtime, RuntimeConfig};
use zo_runtime_render::render::{StateCell, StateValue};
use zo_sir::Insn;
use zo_span::Span;
use zo_ui_protocol::UiCommand;
use zo_value::FunctionKind;

/// Parameters for building reactive event handlers.
struct ReactiveContext<'a> {
  instructions: &'a [Insn],
  interner: &'a zo_interner::Interner,
  handler_names: &'a [String],
  bindings: &'a [(usize, Symbol)],
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

    let (semantic, tokenization, _parsing) =
      compiler.analyze_source(&source, input_path);

    // Extract UI commands and bindings from templates.
    let mut ui_commands = Vec::new();
    let mut template_bindings: Vec<(usize, Symbol)> = Vec::new();
    let mut has_dom_directive = false;

    for insn in &semantic.sir.instructions {
      match insn {
        Insn::Template {
          commands, bindings, ..
        } => {
          let base = ui_commands.len();

          ui_commands.extend_from_slice(commands);

          for (cmd_idx, sym) in bindings {
            template_bindings.push((base + cmd_idx, *sym));
          }
        }
        Insn::Directive { name, .. } => {
          let directive_name = tokenization.interner.get(*name);

          if directive_name == "dom" {
            has_dom_directive = true;
          }
        }
        _ => {}
      }
    }

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

      // Detect reactive template.
      let is_reactive = !template_bindings.is_empty()
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
          interner: &tokenization.interner,
          handler_names: &handler_names,
          bindings: &template_bindings,
          commands: &ui_commands,
          shared_cmds,
        };

        self.build_reactive_handlers(&ctx, &mut event_registry);
      } else {
        self.build_static_handlers(
          &semantic.sir.instructions,
          &tokenization.interner,
          &handler_names,
          &mut event_registry,
        );
      }

      runtime.set_events(event_registry);

      runtime.run().map_err(|_| {
        Error::new(ErrorKind::InternalCompilerError, Span::ZERO)
      })?;
    } else {
      // Programming path: compile to temp binary, execute.
      let temp_path =
        std::env::temp_dir().join(format!("zo_run_{}", std::process::id()));

      compiler.compile(
        &[(input_path, source.clone())],
        self.args.target.into(),
        &[],
        &Some(temp_path.clone()),
      )?;

      let status = std::process::Command::new(&temp_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

      let _ = std::fs::remove_file(&temp_path);

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
    let bindings = ctx.bindings;
    let commands = ctx.commands;
    let shared_cmds = &ctx.shared_cmds;
    // Create state cells for each bound variable.
    // Shared SIR instructions for all handler closures
    // (avoids cloning per handler).
    let sir_arc: std::sync::Arc<Vec<Insn>> =
      std::sync::Arc::new(instructions.to_vec());

    let mut state_slots: Vec<(Symbol, String, StateCell)> = Vec::new();

    for (_cmd_idx, sym) in bindings {
      if state_slots.iter().any(|(s, _, _)| s == sym) {
        continue;
      }

      let var_name = interner.get(*sym).to_string();
      let initial = Self::find_initial_value(instructions, *sym, interner);

      state_slots.push((*sym, var_name, StateCell::new(initial)));
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
      let bindings_copy: Vec<(usize, usize)> = bindings
        .iter()
        .filter_map(|(cmd_idx, sym)| {
          state_slots
            .iter()
            .position(|(s, _, _)| s == sym)
            .map(|slot_idx| (*cmd_idx, slot_idx))
        })
        .collect();
      let commands_copy = commands.to_vec();
      let shared = shared_cmds.clone();
      let sir = sir_arc.clone();
      let closure_sym = *name;

      registry.register(
        fun_name,
        Box::new(move || {
          // 1. Execute the closure body via SIR evaluator.
          let mut eval = zo_runtime_render::evaluator::HandlerEvaluator::new();

          eval.execute(&sir, closure_sym, &cells, &capture_map);

          // 2. Build updated commands from current state.
          let mut new_cmds = commands_copy.clone();

          for &(cmd_idx, slot_idx) in &bindings_copy {
            let value = cells[slot_idx].get().display();

            if let Some(UiCommand::Text { content, .. }) =
              new_cmds.get_mut(cmd_idx)
            {
              *content = value;
            }
          }

          // 3. Push updated commands to the runtime.
          *shared.lock().unwrap() = new_cmds;
        }),
      );
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
    match self.run() {
      Ok(_) => std::process::exit(EXIT_CODE_SUCCESS),
      Err(_) => std::process::exit(EXIT_CODE_ERROR),
    }
  }
}
