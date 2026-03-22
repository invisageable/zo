use crate::args;
use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use zo_compiler::Compiler;
use zo_error::{Error, ErrorKind};
use zo_runtime::{EventRegistry, Graphics, Runtime, RuntimeConfig};
use zo_sir::Insn;
use zo_span::Span;
use zo_ui_protocol::UiCommand;

#[derive(clap::Args, Debug)]
pub(crate) struct Run {
  #[command(flatten)]
  pub(crate) args: args::Args,
  /// Render templates to HTML and open in webview
  #[arg(long)]
  pub(crate) html: bool,
}

impl Run {
  fn run(&self) -> Result<(), Error> {
    // Check for input files
    if self.args.files.is_empty() {
      eprintln!("Error: No input files specified");
      std::process::exit(EXIT_CODE_ERROR);
    }

    let input_path = &self.args.files[0];
    if !input_path.exists() {
      eprintln!("Error: File not found: {}", input_path.display());
      std::process::exit(EXIT_CODE_ERROR);
    }

    // Read source file
    let source = match std::fs::read_to_string(input_path) {
      Ok(c) => c,
      Err(error) => {
        eprintln!("Error reading file {}: {error}", input_path.display());
        std::process::exit(EXIT_CODE_ERROR);
      }
    };

    let search_paths = crate::cmd::search_paths(input_path);

    let mut compiler = Compiler::with_search_paths(search_paths);

    let (semantic, tokenization, _parsing) =
      compiler.analyze_source(&source, input_path);

    // Extract UI commands from templates in SIR
    let mut ui_commands = Vec::new();
    let mut has_dom_directive = false;

    for insn in &semantic.sir.instructions {
      match insn {
        Insn::Template { commands, .. } => {
          ui_commands.extend_from_slice(commands);
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
      let graphics = if self.html {
        Graphics::Web
      } else {
        Graphics::Native
      };

      println!(
        "Running template with {} UI commands ({graphics:?} mode)...",
        ui_commands.len(),
      );

      // Build event registry: collect handler names from Event
      // commands, find matching FunDef in SIR, register handler.
      let mut event_registry = EventRegistry::new();
      let mut handler_names: Vec<String> = Vec::new();

      for cmd in &ui_commands {
        if let UiCommand::Event { handler, .. } = cmd
          && !handler.is_empty()
          && !handler_names.contains(handler)
        {
          handler_names.push(handler.clone());
        }
      }

      // Match handler names to SIR FunDefs and register
      for insn in &semantic.sir.instructions {
        if let Insn::FunDef { name, .. } = insn {
          let fun_name = tokenization.interner.get(*name).to_string();

          if handler_names.contains(&fun_name) {
            let handler_name = fun_name.clone();

            event_registry.register(
              fun_name,
              Box::new(move || {
                println!("[zo] event handler '{handler_name}' called");
              }),
            );
          }
        }
      }

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
      runtime.set_commands(ui_commands);
      runtime.set_events(event_registry);

      runtime.run().map_err(|_| {
        Error::new(ErrorKind::InternalCompilerError, Span::ZERO)
      })?;
    } else {
      // Programming path: compile to temp binary, execute, clean up.
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
}

impl Handle for Run {
  fn handle(&self) {
    match self.run() {
      Ok(_) => std::process::exit(EXIT_CODE_SUCCESS),
      Err(_) => std::process::exit(EXIT_CODE_ERROR),
    }
  }
}
