use crate::args;
use crate::cmd;
use crate::cmd::Handle;

use zo_bundler::macos;
use zo_codegen_backend::Webviewing;
use zo_compiler::{Analyzed, Compiler, DiagnosticsConfig, Stage};
use zo_error::Error;
use zo_interner::Interner;
use zo_sir::Sir;
use zo_ui_protocol::{Attr, ElementTag, PropValue, UiCommand};

use std::path::{Path, PathBuf};

#[derive(clap::Args, Debug)]
pub(crate) struct Build {
  #[command(flatten)]
  pub(crate) args: args::Args,
}

impl Build {
  fn build(&self) -> Result<(), Error> {
    let source_files: Vec<_> = self
      .args
      .files
      .iter()
      .map(|p| (p, cmd::read_source(p)))
      .collect();

    let source = source_files[0].0;
    let mut compiler = self.configured_compiler(source);

    if self.args.target.is_webview() {
      return self.bundle_webview(&mut compiler, &source_files, source);
    }

    let stages = self
      .args
      .emit
      .iter()
      .map(|stage| match stage {
        args::Stage::Tokens => Stage::Tokens,
        args::Stage::Tree => Stage::Tree,
        args::Stage::Sir => Stage::Sir,
        args::Stage::Asm => Stage::Asm,
        args::Stage::All => Stage::All,
      })
      .collect::<Vec<_>>();

    compiler.compile(
      &source_files,
      self.args.target.into(),
      &stages,
      &self.args.output,
      self.args.out_dir.as_deref(),
    )
  }

  /// A compiler carrying this command's search paths and diagnostics settings.
  fn configured_compiler(&self, source: &Path) -> Compiler {
    let mut compiler = Compiler::with_search_paths(cmd::search_paths(source));

    compiler.configure_diagnostics(DiagnosticsConfig {
      format: self.args.format.into(),
      snippet_context: self.args.snippet_context,
      explain_decisions: self.args.explain_decisions,
      use_colors: self.args.use_colors(),
    });

    compiler
  }

  /// Compile the program for the webview runtime, then package the
  /// binary, its runtime dylib, and referenced assets into a
  /// double-clickable `.app`. Analyzes first (like `run --target ios`)
  /// so the referenced images can be collected from the SIR before
  /// codegen consumes it.
  fn bundle_webview(
    &self,
    compiler: &mut Compiler,
    source_files: &[(&PathBuf, String)],
    source: &Path,
  ) -> Result<(), Error> {
    let app = self.app_path(source);

    let Some(name) = app.file_stem().and_then(|s| s.to_str()) else {
      eprintln!("Error: cannot derive an app name from {}", app.display());

      std::process::exit(crate::constants::EXIT_CODE_ERROR);
    };

    compiler.set_webviewing(Webviewing::Yes);

    let (semantic, tokenization, parsing, session, file_table) =
      compiler.analyze_source(&source_files[0].1, source);

    // Collect referenced local images before the analysis is consumed —
    // resolved against the source's directory, the same base the
    // in-process run path uses.
    let base_dir = source.parent().unwrap_or_else(|| Path::new("."));
    let assets = webview_assets(&semantic.sir, &session.interner, base_dir);

    let analyzed = Analyzed {
      semantic,
      tokenization,
      parsing,
      session,
      file_table,
    };

    let staging = std::env::temp_dir()
      .join(format!("zo_build_webview_{}", std::process::id()));
    let binary = staging.join(name);

    let _ = std::fs::create_dir_all(&staging);

    compiler.compile_analyzed(&analyzed, self.args.target.into(), &binary)?;

    let runtime_dylib = staging.join("deps").join("libzo_runtime.dylib");
    let bundle_id = zo_bundler::bundle_id(name);

    let spec = macos::BundleSpec {
      binary: &binary,
      runtime_dylib: &runtime_dylib,
      app_dir: &app,
      name,
      bundle_id: &bundle_id,
      assets: &assets,
    };

    if let Err(error) = macos::bundle(&spec) {
      eprintln!("Error bundling webview app: {error}");

      std::process::exit(crate::constants::EXIT_CODE_ERROR);
    }

    let _ = std::fs::remove_dir_all(&staging);

    eprintln!("zo — built {}", app.display());

    Ok(())
  }

  /// Where the `.app` lands: `-o` names it, else
  /// `<--out-dir>/<source-stem>.app`, else next to the source.
  fn app_path(&self, source: &Path) -> PathBuf {
    let base = match (&self.args.output, self.args.out_dir.as_deref()) {
      (Some(out), _) => out.clone(),
      (None, Some(dir)) => {
        let stem = source.file_stem().unwrap_or(source.as_os_str());
        dir.join(stem)
      }
      (None, None) => source.with_extension(""),
    };

    base.with_extension("app")
  }
}

/// The program's referenced local image files — `<img>` srcs and CSS
/// `background-image`s — resolved to absolute paths against `base_dir`.
/// Remote URLs and missing files are skipped. The bundler copies these
/// into the `.app`'s resources.
fn webview_assets(
  sir: &Sir,
  interner: &Interner,
  base_dir: &Path,
) -> Vec<PathBuf> {
  let mut assets: Vec<PathBuf> = Vec::new();

  for command in sir.ui_commands(interner) {
    let UiCommand::Element {
      tag: ElementTag::Img,
      attrs,
      ..
    } = command
    else {
      continue;
    };

    for attr in &attrs {
      let value = match attr {
        Attr::Prop { name, value } if name == "src" => value,
        Attr::Dynamic { name, initial, .. } if name == "src" => initial,
        _ => continue,
      };

      if let PropValue::Str(src) = value {
        push_asset(src, base_dir, &mut assets);
      }
    }
  }

  for css in sir.stylesheets() {
    for url in zo_ui_protocol::style::css::parse(css).images {
      push_asset(&url, base_dir, &mut assets);
    }
  }

  assets
}

/// Resolve `src` against `base_dir` and add it to `assets` when it's a
/// local file that exists (skipping remote URLs and duplicates).
fn push_asset(src: &str, base_dir: &Path, assets: &mut Vec<PathBuf>) {
  if src.starts_with("http://") || src.starts_with("https://") {
    return;
  }

  let path = Path::new(src);
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    base_dir.join(path)
  };

  if absolute.is_file() && !assets.contains(&absolute) {
    assets.push(absolute);
  }
}

impl Handle for Build {
  fn handle(&self) {
    cmd::handle_with_watch(&self.args, || self.build());
  }
}
