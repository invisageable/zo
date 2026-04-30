//! Emit the three-language synth_10k bench inputs from a
//! single source-of-truth pattern: `synth_10k.zo`,
//! `synth_10k.c`, `synth_10k.rs`. Each file is the same
//! 1000-function chain — `func_i` calls `func_(i-1)`, and
//! main computes `func_999(1)`. Compile time across
//! compilers is what we measure.
//!
//! Names are zero-padded `funcNNN` so they never collide
//! with zo's type keywords (`f32`, `f64`, etc.).
//!
//! Compiled as a standalone rustc binary (sibling
//! `generate_test.rs` follows the same convention):
//! ```
//! cd crates/compiler/zo-tasks && \
//!   rustc generate_synth.rs --edition=2021 -O \
//!         -o generate_synth && \
//!   ./generate_synth
//! ```
//!
//! Outputs land at
//! `../zo-benches/benches/synth_10k/synth_10k.{zo,c,rs}`.

use std::fs;
use std::path::PathBuf;

const NUM_FUNCTIONS: usize = 1000;

fn emit_zo() -> String {
  let mut s = String::with_capacity(150 * 1024);

  s.push_str("-- generated bench source. ~10K lines.\n\n");
  s.push_str("fun func000(x: int) -> int { return x + 1; }\n\n");

  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "fun func{i:03}(x: int) -> int {{\n  imu y: int = func{prev:03}(x);\n  imu z: int = y * 2 - 1;\n  if z > 0 {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  s.push_str(&format!(
    "fun main() {{\n  imu r: int = func{:03}(1);\n  showln(r);\n}}\n",
    NUM_FUNCTIONS - 1
  ));

  s
}

fn emit_c() -> String {
  let mut s = String::with_capacity(150 * 1024);

  s.push_str("// generated bench source. ~10K lines.\n\n");
  s.push_str("#include <stdio.h>\n\n");
  s.push_str("long func000(long x) { return x + 1; }\n\n");

  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "long func{i:03}(long x) {{\n  long y = func{prev:03}(x);\n  long z = y * 2 - 1;\n  if (z > 0) {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  s.push_str(&format!(
    "int main(void) {{\n  long r = func{:03}(1);\n  printf(\"%ld\\n\", r);\n  return 0;\n}}\n",
    NUM_FUNCTIONS - 1
  ));

  s
}

fn emit_rust() -> String {
  let mut s = String::with_capacity(150 * 1024);

  s.push_str("// generated bench source. ~10K lines.\n");
  s.push_str("#![allow(dead_code)]\n\n");
  s.push_str("fn func000(x: i64) -> i64 { x + 1 }\n\n");

  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "fn func{i:03}(x: i64) -> i64 {{\n  let y: i64 = func{prev:03}(x);\n  let z: i64 = y * 2 - 1;\n  if z > 0 {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  s.push_str(&format!(
    "fn main() {{\n  let r: i64 = func{:03}(1);\n  println!(\"{{}}\", r);\n}}\n",
    NUM_FUNCTIONS - 1
  ));

  s
}

fn main() {
  // Run from `crates/compiler/zo-tasks/` (sibling of
  // `zo-benches`) so this relative path resolves; matches
  // how `generate_test.rs` is invoked.
  let dir = PathBuf::from("../zo-benches/benches/synth_10k");

  fs::create_dir_all(&dir).unwrap();

  let zo_path = dir.join("synth_10k.zo");
  let c_path = dir.join("synth_10k.c");
  let rs_path = dir.join("synth_10k.rs");

  let zo = emit_zo();
  let c = emit_c();
  let rs = emit_rust();

  fs::write(&zo_path, &zo).unwrap();
  fs::write(&c_path, &c).unwrap();
  fs::write(&rs_path, &rs).unwrap();

  println!(
    "wrote {} ({} lines, {} bytes)",
    zo_path.display(),
    zo.lines().count(),
    zo.len()
  );

  println!(
    "wrote {} ({} lines, {} bytes)",
    c_path.display(),
    c.lines().count(),
    c.len()
  );

  println!(
    "wrote {} ({} lines, {} bytes)",
    rs_path.display(),
    rs.lines().count(),
    rs.len()
  );
}
