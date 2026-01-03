use std::process::Command;
use zo_codegen_arm::ARM64Gen;

#[test]
fn test_signed_binary() {
  let binary = ARM64Gen::generate_hello_world_signed();

  assert!(!binary.is_empty());
  assert_eq!(&binary[0..4], &[0xCF, 0xFA, 0xED, 0xFE]);

  let path = "tests/hello_signed";

  ARM64Gen::write_executable(binary, path).expect("Failed to write executable");

  let output = Command::new(path)
    .output()
    .expect("Failed to execute binary");

  let stdout = String::from_utf8_lossy(&output.stdout);

  assert_eq!(stdout, "Hello, World!\n");
  assert!(output.status.success());
}

#[test]
fn test_code_signature_structure() {
  let mut macho = zo_writer_macho::MachO::new();

  macho.add_code(vec![0xC0, 0x03, 0x5F, 0xD6]);

  let binary = macho.finish_with_signature();

  assert!(binary.len() > 1024);

  // Look for code signature magic numbers
  // Note: They're in big-endian in the file
  let sig_magic = &[0xFA, 0xDE, 0x0C, 0xC0]; // CSMAGIC_EMBEDDED_SIGNATURE
  let found_sig = binary.windows(4).any(|w| w == sig_magic);

  assert!(found_sig, "Code signature magic not found");

  let dir_magic = &[0xFA, 0xDE, 0x0C, 0x02]; // CSMAGIC_CODEDIRECTORY  
  let found_dir = binary.windows(4).any(|w| w == dir_magic);

  assert!(found_dir, "Code directory magic not found");
}
