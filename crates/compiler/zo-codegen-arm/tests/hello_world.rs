use std::process::Command;
use zo_codegen_arm::ARM64Gen;

#[test]
fn test_hello_world_generation() {
  // Generate the Hello World binary
  let binary = ARM64Gen::generate_hello_world();

  // Binary should be non-empty
  assert!(!binary.is_empty());

  // Should have Mach-O magic number
  assert_eq!(&binary[0..4], &[0xCF, 0xFA, 0xED, 0xFE]); // MH_MAGIC_64 in little-endian

  // Write to file in tests directory
  let path = "tests/hello_zo";

  ARM64Gen::write_executable(binary, path).expect("Failed to write executable");

  // Sign the binary (required on macOS Apple Silicon)
  let sign_output = Command::new("codesign")
    .args(&["-s", "-", path])
    .output()
    .expect("Failed to sign binary");

  if !sign_output.status.success() {
    panic!(
      "Failed to sign binary: {}",
      String::from_utf8_lossy(&sign_output.stderr)
    );
  }

  // Try to execute it
  let output = Command::new(path)
    .output()
    .expect("Failed to execute binary");

  // Check output
  let stdout = String::from_utf8_lossy(&output.stdout);

  assert_eq!(stdout, "Hello, World!\n");

  // Check exit code
  assert!(output.status.success());
}

#[test]
fn test_binary_structure() {
  let binary = ARM64Gen::generate_hello_world();

  // Check minimum size (at least header + load commands + code)
  assert!(binary.len() >= 0x4000);

  // Check CPU type (ARM64)
  let cputype =
    u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);

  assert_eq!(cputype, 0x0100000C); // CPU_TYPE_ARM64

  // Check file type (executable)
  let filetype =
    u32::from_le_bytes([binary[12], binary[13], binary[14], binary[15]]);

  assert_eq!(filetype, 0x00000002); // MH_EXECUTE
}
