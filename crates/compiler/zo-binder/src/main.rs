//! zo-binder CLI — regenerate a provider's `.zo` bindings
//! from its Rust shim crate.
//!
//! @note — `zo-binder <lib>` reads
//! `crates/compiler/zo-provider-<lib>/src/lib.rs` and writes
//! `crates/compiler-lib/provider/<lib>/<lib>.zo`. Run from the
//! workspace root (via `just bind <lib>`).

use zo_binder::bind::bind;

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
  let Some(lib) = std::env::args().nth(1) else {
    eprintln!("usage: zo-binder <lib>");
    return ExitCode::FAILURE;
  };

  match run(&lib) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("zo-binder: {error}");
      ExitCode::FAILURE
    }
  }
}

/// Read the shim, render its bindings, and write on change.
fn run(lib: &str) -> Result<(), String> {
  let input =
    PathBuf::from(format!("crates/compiler/zo-provider-{lib}/src/lib.rs"));

  let dir = PathBuf::from(format!("crates/compiler-lib/provider/{lib}"));
  let output = dir.join(format!("{lib}.zo"));

  let src = fs::read_to_string(&input)
    .map_err(|error| format!("read {}: {error}", input.display()))?;

  let generated = bind(lib, &src).map_err(|error| error.to_string())?;

  if fs::read_to_string(&output).is_ok_and(|old| old == generated) {
    println!("up to date: {}", output.display());
    return Ok(());
  }

  fs::create_dir_all(&dir)
    .map_err(|error| format!("mkdir {}: {error}", dir.display()))?;
  fs::write(&output, &generated)
    .map_err(|error| format!("write {}: {error}", output.display()))?;

  println!("wrote: {}", output.display());
  Ok(())
}
