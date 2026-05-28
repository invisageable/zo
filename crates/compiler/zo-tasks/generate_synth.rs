//! Emit the four-language stress_fun_10k bench inputs from
//! a single source-of-truth pattern:
//! `stress_fun_10k.{zo,c,go,rs,odin}`. Each file is the same
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
//! `../zo-benches/benches/stress_fun_10k/stress_fun_10k.{zo,c,go,rs,odin}`.

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
  s.push_str("fn func000(x: i64) -> i64 { x.wrapping_add(1) }\n\n");

  // The chain doubles each step, so after ~63 hops the
  // value crosses i64::MAX. C/zo/Odin all wrap silently
  // by default; Rust debug builds panic on overflow.
  // Use `wrapping_*` ops so all four languages reach
  // `func999(1)` with identical semantics.
  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "fn func{i:03}(x: i64) -> i64 {{\n  let y: i64 = func{prev:03}(x);\n  let z: i64 = y.wrapping_mul(2).wrapping_sub(1);\n  if z > 0 {{\n    return z;\n  }} else {{\n    return z.wrapping_neg();\n  }}\n}}\n\n",
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

fn emit_go() -> String {
  let mut s = String::with_capacity(150 * 1024);

  s.push_str("// generated bench source. ~10K lines.\n\n");
  s.push_str("package main\n\n");
  s.push_str("import \"fmt\"\n\n");
  s.push_str("func func000(x int64) int64 { return x + 1 }\n\n");

  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "func func{i:03}(x int64) int64 {{\n\ty := func{prev:03}(x)\n\tz := y*2 - 1\n\tif z > 0 {{\n\t\treturn z\n\t}} else {{\n\t\treturn -z\n\t}}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  s.push_str(&format!(
    "func main() {{\n\tr := func{:03}(1)\n\tfmt.Println(r)\n}}\n",
    NUM_FUNCTIONS - 1
  ));

  s
}

fn emit_odin() -> String {
  let mut s = String::with_capacity(150 * 1024);

  s.push_str("// generated bench source. ~10K lines.\n\n");
  s.push_str("package main\n\n");
  s.push_str("import \"core:fmt\"\n\n");
  s.push_str("func000 :: proc(x: i64) -> i64 { return x + 1 }\n\n");

  for i in 1..NUM_FUNCTIONS {
    s.push_str(&format!(
      "func{i:03} :: proc(x: i64) -> i64 {{\n  y: i64 = func{prev:03}(x)\n  z: i64 = y * 2 - 1\n  if z > 0 {{\n    return z\n  }} else {{\n    return -z\n  }}\n}}\n\n",
      i = i,
      prev = i - 1
    ));
  }

  s.push_str(&format!(
    "main :: proc() {{\n  r: i64 = func{:03}(1)\n  fmt.println(r)\n}}\n",
    NUM_FUNCTIONS - 1
  ));

  s
}

fn main() {
  // Run from `crates/compiler/zo-tasks/` (sibling of
  // `zo-benches`) so this relative path resolves; matches
  // how `generate_test.rs` is invoked.
  let dir = PathBuf::from("../zo-benches/benches/stress_fun_10k");

  fs::create_dir_all(&dir).unwrap();

  let c_path = dir.join("stress_fun_10k.c");
  let go_path = dir.join("stress_fun_10k.go");
  let odin_path = dir.join("stress_fun_10k.odin");
  let rs_path = dir.join("stress_fun_10k.rs");
  let zo_path = dir.join("stress_fun_10k.zo");

  let c = emit_c();
  let go = emit_go();
  let odin = emit_odin();
  let rs = emit_rust();
  let zo = emit_zo();

  fs::write(&c_path, &c).unwrap();
  fs::write(&go_path, &go).unwrap();
  fs::write(&odin_path, &odin).unwrap();
  fs::write(&rs_path, &rs).unwrap();
  fs::write(&zo_path, &zo).unwrap();

  for (path, src) in [
    (&c_path, &c),
    (&go_path, &go),
    (&odin_path, &odin),
    (&rs_path, &rs),
    (&zo_path, &zo),
  ] {
    println!(
      "wrote {} ({} lines, {} bytes)",
      path.display(),
      src.lines().count(),
      src.len()
    );
  }
}
