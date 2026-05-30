//! zo-binder CLI — regenerate a provider's `.zo` bindings.
//!
//! @note — run from the workspace root, in one of two modes.
//!
//! Rust shim: `zo-binder <lib>` reads
//! `crates/compiler/zo-provider-<lib>/src/lib.rs`.
//!
//! C header: `zo-binder --json <api.json> --lib <name>
//! --macos <path> --linux <path> [--out <path>]` reads
//! rlparser JSON.
//!
//! Both write `provider/<lib>/<lib>.zo` unless `--out` is set.

use zo_binder::bind::bind;
use zo_binder::cbind::bind_c_api;
use zo_binder::cheader::parse_c_api;
use zo_binder::model::LinkSpec;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();

  let result = match args.first() {
    Some(first) if !first.starts_with("--") => run_shim(first),
    Some(_) => run_c(&flags(&args)),
    None => Err(
      "usage: zo-binder <lib> | zo-binder --json <api.json> \
       --lib <name> --macos <path> --linux <path> [--out <path>]"
        .to_string(),
    ),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("zo-binder: {error}");
      ExitCode::FAILURE
    }
  }
}

/// Generate from a Rust shim crate's `src/lib.rs`.
fn run_shim(lib: &str) -> Result<(), String> {
  let input =
    PathBuf::from(format!("crates/compiler/zo-provider-{lib}/src/lib.rs"));

  let source = fs::read_to_string(&input)
    .map_err(|error| format!("read {}: {error}", input.display()))?;

  let generated = bind(lib, &source).map_err(|error| error.to_string())?;

  write_if_changed(&provider_path(lib), &generated)
}

/// Generate from an rlparser `raylib_api.json`.
fn run_c(flags: &HashMap<String, String>) -> Result<(), String> {
  let json = flags.get("json").ok_or("missing --json <api.json>")?;
  let lib = flags.get("lib").ok_or("missing --lib <name>")?;
  let macos = flags.get("macos").ok_or("missing --macos <path>")?;
  let linux = flags.get("linux").ok_or("missing --linux <path>")?;

  let source = fs::read_to_string(json)
    .map_err(|error| format!("read {json}: {error}"))?;
  let api = parse_c_api(&source).map_err(|error| error.to_string())?;

  let link = LinkSpec::System {
    macos: macos.clone(),
    linux: linux.clone(),
  };
  let result = bind_c_api(lib, link, &api);

  let output = match flags.get("out") {
    Some(path) => PathBuf::from(path),
    None => provider_path(lib),
  };
  write_if_changed(&output, &result.output)?;

  if !result.skipped.is_empty() {
    println!(
      "skipped {} unsupported item(s): {}",
      result.skipped.len(),
      result.skipped.join(", ")
    );
  }

  Ok(())
}

/// Parse `--key value` pairs into a map.
///
/// @note — a value that itself begins with `--` means the flag
/// was given none; leave it unset so `run_c` reports the
/// omission instead of swallowing the next flag as a value.
fn flags(args: &[String]) -> HashMap<String, String> {
  let mut map = HashMap::new();
  let mut iter = args.iter().peekable();

  while let Some(arg) = iter.next() {
    let Some(key) = arg.strip_prefix("--") else {
      continue;
    };

    if iter.peek().is_some_and(|value| !value.starts_with("--")) {
      map.insert(key.to_string(), iter.next().unwrap().clone());
    }
  }

  map
}

/// The committed binding path for `lib`.
fn provider_path(lib: &str) -> PathBuf {
  PathBuf::from(format!("crates/compiler-lib/provider/{lib}/{lib}.zo"))
}

/// Write `content` to `output`, skipping an unchanged file.
fn write_if_changed(output: &Path, content: &str) -> Result<(), String> {
  if fs::read_to_string(output).is_ok_and(|old| old == content) {
    println!("up to date: {}", output.display());
    return Ok(());
  }

  if let Some(dir) = output.parent() {
    fs::create_dir_all(dir)
      .map_err(|error| format!("mkdir {}: {error}", dir.display()))?;
  }

  fs::write(output, content)
    .map_err(|error| format!("write {}: {error}", output.display()))?;

  println!("wrote: {}", output.display());
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::flags;

  /// `--key value` pairs parse into a map.
  #[test]
  fn parses_key_value_pairs() {
    let args = ["--json", "api.json", "--lib", "raylib"].map(String::from);
    let map = flags(&args);

    assert_eq!(map.get("json").map(String::as_str), Some("api.json"));
    assert_eq!(map.get("lib").map(String::as_str), Some("raylib"));
  }

  /// A flag missing its value does not swallow the next flag.
  #[test]
  fn missing_value_does_not_swallow_next_flag() {
    let args = ["--json", "--lib", "raylib"].map(String::from);
    let map = flags(&args);

    assert_eq!(map.get("json"), None);
    assert_eq!(map.get("lib").map(String::as_str), Some("raylib"));
  }

  /// A trailing flag with no value is dropped, not panicking.
  #[test]
  fn trailing_flag_without_value_is_dropped() {
    let args = ["--lib", "raylib", "--out"].map(String::from);
    let map = flags(&args);

    assert_eq!(map.get("lib").map(String::as_str), Some("raylib"));
    assert_eq!(map.get("out"), None);
  }
}
