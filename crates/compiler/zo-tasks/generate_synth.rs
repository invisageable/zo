//! Emit four-language stress-bench inputs from a single
//! source-of-truth pattern: `<name>.{zo,c,go,rs,odin}`. Each
//! file is the same function chain — `funcN` calls
//! `func(N-1)`, and main computes `func(last)(1)`. Compile
//! time across compilers is what we measure.
//!
//! Function names are zero-padded `funcNNN` so they never
//! collide with zo's type keywords (`f32`, `f64`, etc.). The
//! pad width scales with the function count.
//!
//! Compiled as a standalone rustc binary (sibling
//! `generate_test.rs` follows the same convention):
//! ```
//! cd crates/compiler/zo-tasks && \
//!   rustc generate_synth.rs --edition=2021 -O \
//!         -o generate_synth && \
//!   ./generate_synth [lines] [name]
//! ```
//!
//! Args (both optional, positional):
//!   lines — target line count. Function count is `lines/10`
//!           (each function is ~10 lines). Default 10000.
//!   name  — output directory + file stem. Default
//!           `stress_fun_10k`.
//!
//! Outputs land at
//! `../zo-benches/benches/<name>/<name>.{zo,c,go,rs,odin}`.
//!
//! The default invocation reproduces the committed
//! `stress_fun_10k` files byte-for-byte (1000 functions,
//! 3-digit padding).

use std::fs;
use std::path::PathBuf;

/// Approximate lines emitted per chained function.
const LINES_PER_FUNCTION: usize = 10;

fn emit_zo(count: usize, pad: usize) -> String {
  let mut s = String::with_capacity(count * 256);

  s.push_str("-- generated bench source.\n\n");
  s.push_str(&format!(
    "fun func{first:0pad$}(x: int) -> int {{ return x + 1; }}\n\n",
    first = 0,
    pad = pad,
  ));

  for i in 1..count {
    s.push_str(&format!(
      "fun func{i:0pad$}(x: int) -> int {{\n  imu y: int = func{prev:0pad$}(x);\n  imu z: int = y * 2 - 1;\n  if z > 0 {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1,
      pad = pad,
    ));
  }

  s.push_str(&format!(
    "fun main() {{\n  imu r: int = func{last:0pad$}(1);\n  showln(r);\n}}\n",
    last = count - 1,
    pad = pad,
  ));

  s
}

fn emit_c(count: usize, pad: usize) -> String {
  let mut s = String::with_capacity(count * 256);

  s.push_str("// generated bench source.\n\n");
  s.push_str("#include <stdio.h>\n\n");
  s.push_str(&format!(
    "long func{first:0pad$}(long x) {{ return x + 1; }}\n\n",
    first = 0,
    pad = pad,
  ));

  for i in 1..count {
    s.push_str(&format!(
      "long func{i:0pad$}(long x) {{\n  long y = func{prev:0pad$}(x);\n  long z = y * 2 - 1;\n  if (z > 0) {{\n    return z;\n  }} else {{\n    return -z;\n  }}\n}}\n\n",
      i = i,
      prev = i - 1,
      pad = pad,
    ));
  }

  s.push_str(&format!(
    "int main(void) {{\n  long r = func{last:0pad$}(1);\n  printf(\"%ld\\n\", r);\n  return 0;\n}}\n",
    last = count - 1,
    pad = pad,
  ));

  s
}

fn emit_rust(count: usize, pad: usize) -> String {
  let mut s = String::with_capacity(count * 256);

  s.push_str("// generated bench source.\n");
  s.push_str("#![allow(dead_code)]\n\n");
  s.push_str(&format!(
    "fn func{first:0pad$}(x: i64) -> i64 {{ x.wrapping_add(1) }}\n\n",
    first = 0,
    pad = pad,
  ));

  // The chain doubles each step, so after ~63 hops the
  // value crosses i64::MAX. C/zo/Odin all wrap silently
  // by default; Rust debug builds panic on overflow.
  // Use `wrapping_*` ops so all four languages reach
  // `func(last)(1)` with identical semantics.
  for i in 1..count {
    s.push_str(&format!(
      "fn func{i:0pad$}(x: i64) -> i64 {{\n  let y: i64 = func{prev:0pad$}(x);\n  let z: i64 = y.wrapping_mul(2).wrapping_sub(1);\n  if z > 0 {{\n    return z;\n  }} else {{\n    return z.wrapping_neg();\n  }}\n}}\n\n",
      i = i,
      prev = i - 1,
      pad = pad,
    ));
  }

  s.push_str(&format!(
    "fn main() {{\n  let r: i64 = func{last:0pad$}(1);\n  println!(\"{{}}\", r);\n}}\n",
    last = count - 1,
    pad = pad,
  ));

  s
}

fn emit_go(count: usize, pad: usize) -> String {
  let mut s = String::with_capacity(count * 256);

  s.push_str("// generated bench source.\n\n");
  s.push_str("package main\n\n");
  s.push_str("import \"fmt\"\n\n");
  s.push_str(&format!(
    "func func{first:0pad$}(x int64) int64 {{ return x + 1 }}\n\n",
    first = 0,
    pad = pad,
  ));

  for i in 1..count {
    s.push_str(&format!(
      "func func{i:0pad$}(x int64) int64 {{\n\ty := func{prev:0pad$}(x)\n\tz := y*2 - 1\n\tif z > 0 {{\n\t\treturn z\n\t}} else {{\n\t\treturn -z\n\t}}\n}}\n\n",
      i = i,
      prev = i - 1,
      pad = pad,
    ));
  }

  s.push_str(&format!(
    "func main() {{\n\tr := func{last:0pad$}(1)\n\tfmt.Println(r)\n}}\n",
    last = count - 1,
    pad = pad,
  ));

  s
}

fn emit_odin(count: usize, pad: usize) -> String {
  let mut s = String::with_capacity(count * 256);

  s.push_str("// generated bench source.\n\n");
  s.push_str("package main\n\n");
  s.push_str("import \"core:fmt\"\n\n");
  s.push_str(&format!(
    "func{first:0pad$} :: proc(x: i64) -> i64 {{ return x + 1 }}\n\n",
    first = 0,
    pad = pad,
  ));

  for i in 1..count {
    s.push_str(&format!(
      "func{i:0pad$} :: proc(x: i64) -> i64 {{\n  y: i64 = func{prev:0pad$}(x)\n  z: i64 = y * 2 - 1\n  if z > 0 {{\n    return z\n  }} else {{\n    return -z\n  }}\n}}\n\n",
      i = i,
      prev = i - 1,
      pad = pad,
    ));
  }

  s.push_str(&format!(
    "main :: proc() {{\n  r: i64 = func{last:0pad$}(1)\n  fmt.println(r)\n}}\n",
    last = count - 1,
    pad = pad,
  ));

  s
}

fn main() {
  let args: Vec<String> = std::env::args().collect();

  let lines: usize = args
    .get(1)
    .and_then(|a| a.parse().ok())
    .unwrap_or(10_000);

  let name: String =
    args.get(2).cloned().unwrap_or_else(|| "stress_fun_10k".to_string());

  let count = (lines / LINES_PER_FUNCTION).max(2);
  let pad = (count - 1).to_string().len();
  let dir = PathBuf::from("../zo-benches/benches").join(&name);

  fs::create_dir_all(&dir).unwrap();

  let c_path = dir.join(format!("{name}.c"));
  let go_path = dir.join(format!("{name}.go"));
  let odin_path = dir.join(format!("{name}.odin"));
  let rs_path = dir.join(format!("{name}.rs"));
  let zo_path = dir.join(format!("{name}.zo"));

  let c = emit_c(count, pad);
  let go = emit_go(count, pad);
  let odin = emit_odin(count, pad);
  let rs = emit_rust(count, pad);
  let zo = emit_zo(count, pad);

  fs::write(&c_path, &c).unwrap();
  fs::write(&go_path, &go).unwrap();
  fs::write(&odin_path, &odin).unwrap();
  fs::write(&rs_path, &rs).unwrap();
  fs::write(&zo_path, &zo).unwrap();

  println!("{count} functions, {pad}-digit padding, dir {}", dir.display());

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
