fn main() {
  let args = std::env::args().collect::<Vec<_>>();

  let num_functions = if args.len() > 1 {
    args[1].parse::<usize>().unwrap_or(100)
  } else {
    100
  };

  let mut source = String::new();
  source.push_str("-- generated test file.\n\n");

  for i in 1..=num_functions {
    source.push_str(&format!("fun func{i:03}(x: int) -> int {{\n"));
    source.push_str(&format!("  imu a: int = x + {i};\n"));
    source.push_str(&format!("  imu b: int = a * 2;\n"));
    source.push_str(&format!("  return b + {};\n", i * 10));
    source.push_str("}\n\n");
  }

  source.push_str("fun main() -> int {\n");
  source.push_str("  imu result: int = 0;\n");

  for i in 1..=num_functions {
    source.push_str(&format!("  imu result{i} := result + func{i:03}({i});\n"));
  }

  source.push_str("  return result;\n");
  source.push_str("}\n");

  let filename = format!("test_{num_functions}_funcs.zo");

  std::fs::write(&filename, source).unwrap();

  println!(
    "Generated {filename} with {} functions",
    num_functions + 1
  );
}
