use crate::args;
use crate::cmd::Handle;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_SUCCESS};

use zo_analyzer::Analyzer;
use zo_error::{Error, ErrorKind};
use zo_parser::Parser;
use zo_runtime::{Graphics, Runtime, RuntimeConfig};
use zo_sir::Insn;
use zo_span::Span;
use zo_tokenizer::Tokenizer;

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

    // Tokenize
    let tokenizer = Tokenizer::new(&source);
    let tokenization = tokenizer.tokenize();

    // Parse
    let parser = Parser::new(&tokenization, &source);
    let parsing = parser.parse();

    // Analyze
    let analyzer = Analyzer::new(
      &parsing.tree,
      &tokenization.interner,
      &tokenization.literals,
    );
    let semantic = analyzer.analyze();

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

    // Debug: print what we found
    eprintln!(
      "DEBUG: has_dom_directive={has_dom_directive}, ui_commands.len()={}",
      ui_commands.len()
    );
    for (i, cmd) in ui_commands.iter().enumerate() {
      eprintln!("DEBUG: Command {i}: {cmd:?}");
    }

    // Run UI if we have templates with #dom directive
    if has_dom_directive && !ui_commands.is_empty() {
      let graphics = if self.html {
        Graphics::Web
      } else {
        Graphics::Native
      };

      println!(
        "Running template with {} UI commands ({:?} mode)...",
        ui_commands.len(),
        graphics
      );

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

      runtime.run().map_err(|_| {
        Error::new(ErrorKind::InternalCompilerError, Span::ZERO)
      })?;
    } else if !ui_commands.is_empty() {
      println!("Template found but no #dom directive - not launching UI");
    } else {
      // Regular program - just compile and run (future: execute non-UI code)
      println!("No UI templates found - program compiled successfully");
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
