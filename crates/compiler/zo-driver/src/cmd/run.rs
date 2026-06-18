use crate::args;
use crate::cmd::Handle;
use crate::cmd::build::bundle_webview_app;
use crate::constants::{EXIT_CODE_ERROR, EXIT_CODE_USAGE};

use zo_bundler::ios::simulator::Simulator;
use zo_compiler::{Analyzed, Compiler};
use zo_error::Error;
use zo_runtime::Server;

#[derive(clap::Args, Debug)]
pub(crate) struct Run {
  #[command(flatten)]
  pub(crate) args: args::Args,
  /// The Simulator device for `run --target ios|watchos` — a device
  /// name (e.g. "Apple Vision Pro") or UDID, resolved against the
  /// devices the machine actually has. Omitted: the booted device
  /// wins, else the newest iPhone (or watch, for `watchos`).
  #[arg(long)]
  pub(crate) device: Option<String>,
}

impl Run {
  fn run(&self) -> Result<(), Error> {
    if self.args.files.is_empty() {
      eprintln!("Error: No input files specified");
      std::process::exit(EXIT_CODE_USAGE);
    }

    let input_path = &self.args.files[0];
    let source = crate::cmd::read_source(input_path);
    let search_paths = crate::cmd::search_paths(input_path);

    let mut compiler = Compiler::with_search_paths(search_paths);
    compiler.configure_diagnostics(zo_compiler::DiagnosticsConfig {
      format: self.args.format.into(),
      snippet_context: self.args.snippet_context,
      explain_decisions: self.args.explain_decisions,
      use_colors: self.args.use_colors(),
      quiet: self.args.quiet,
    });

    // The webview bundler sets the `webviewing` flag and owns its own
    // analysis (its codegen differs), so it runs before the shared
    // analysis below.
    if self.args.target.is_webview() {
      return self.run_webview(&mut compiler, input_path, &source);
    }

    let (semantic, tokenization, parsing, session, file_table) =
      compiler.analyze_source(&source, input_path);

    let analyzed = Analyzed {
      semantic,
      tokenization,
      parsing,
      session,
      file_table,
    };

    // Every target runs the compiled artifact — `zo run` and `zo build`
    // share one execution semantics. iOS/watchOS and web hand the
    // artifact to their own Runtimer; native compiles to a temp binary
    // and execs it. A `#render` program opens its own window from that
    // binary; a CLI program runs to completion.
    if self.args.target.is_ios() || self.args.target.is_watchos() {
      return self.run_simulator(&mut compiler, &analyzed, input_path);
    }

    if self.args.target.is_web() {
      return self.run_web(&mut compiler, &analyzed);
    }

    // Native: compile to a per-run isolated dir, then exec. The
    // isolation matters — codegen emits `LC_LOAD_DYLIB
    // @loader_path/deps/libzo_runtime.dylib` and the compiler stages
    // the dylib into a sibling `deps/`. A flat shared `temp_dir()`
    // would let one run overwrite the dylib while an earlier run (or a
    // dyld-stuck zombie) still holds it open — wedging the new run in
    // `dyld3::MachOFile::compatibleSlice` indefinitely. A fresh
    // subdirectory per run sidesteps the whole class.
    let run_dir =
      std::env::temp_dir().join(format!("zo_run_{}", std::process::id()));

    let _ = std::fs::create_dir_all(&run_dir);

    let temp_path = run_dir.join("a.out");

    compiler.compile_analyzed(&analyzed, self.args.target.into(), &temp_path)?;

    // Run with cwd set to the source file's parent so relative paths in
    // `read_file(...)` etc. resolve against the program's own directory
    // regardless of where `zo run` was invoked from.
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

    Ok(())
  }

  /// Build the iOS/watchOS `.app` from the already-analyzed program
  /// and hand it to the Simulator: boot, install, launch.
  fn run_simulator(
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

    // Resolve the device against the machine's actual simulators
    // before the build — a bad `--device` should fail fast, not after
    // codegen.
    let devices = match zo_bundler::ios::device::detect() {
      Ok(devices) => devices,
      Err(error) => {
        eprintln!("Error listing Simulator devices: {error}");

        std::process::exit(EXIT_CODE_ERROR);
      }
    };

    // An empty `--device ""` means auto-select.
    let requested = self.device.as_deref().filter(|name| !name.is_empty());

    let artifact = if self.args.target.is_watchos() {
      zo_bundler::ios::device::Artifact::Watchos
    } else {
      zo_bundler::ios::device::Artifact::Ios
    };

    let device =
      match zo_bundler::ios::device::resolve(&devices, requested, artifact) {
        Ok(device) => device,
        Err(error) => {
          eprintln!("Error: {error}");

          std::process::exit(EXIT_CODE_ERROR);
        }
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
    let simulator = Simulator::new(&device.udid);

    if !self.args.quiet {
      eprintln!(
        "Launching on {} ({} {})...",
        device.name, device.os, device.os_version,
      );
    }

    if let Err(error) = simulator.launch(&app, &bundle_id) {
      eprintln!("Error launching Simulator: {error}");

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

    let server = Server::new(&public).with_quiet(self.args.quiet.into());
    if let Err(error) = server.serve() {
      eprintln!("Error serving web bundle: {error}");

      std::process::exit(EXIT_CODE_ERROR);
    }

    Ok(())
  }

  /// Build the webview `.app` from this program, then launch it. Shares
  /// the bundler with `build --target webview`; `run` opens the app
  /// once it is built.
  fn run_webview(
    &self,
    compiler: &mut Compiler,
    input_path: &std::path::Path,
    source: &str,
  ) -> Result<(), Error> {
    let app = bundle_webview_app(compiler, &self.args, input_path, source)?;

    if !self.args.quiet {
      eprintln!("Launching {}...", app.display());
    }

    if let Err(error) = std::process::Command::new("open").arg(&app).status() {
      eprintln!("Error launching webview app: {error}");

      std::process::exit(EXIT_CODE_ERROR);
    }

    Ok(())
  }
}

impl Handle for Run {
  fn handle(&self) {
    crate::cmd::handle_with_watch(&self.args, || self.run());
  }
}
