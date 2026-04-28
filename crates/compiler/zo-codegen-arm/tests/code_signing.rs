use zo_codegen_arm::ARM64Gen;

use std::process::Command;

#[test]
#[ignore = "code signing implementation incomplete - binary execution fails"]
fn test_signed_binary() {
  let binary = ARM64Gen::generate_hello_world_signed();

  assert!(!binary.is_empty());
  assert_eq!(&binary[0..4], &[0xCF, 0xFA, 0xED, 0xFE]);

  let path = "tests/bin/hello_signed";

  zo_linker::write_executable(&binary, std::path::Path::new(path))
    .expect("Failed to write executable");

  let output = Command::new(path)
    .output()
    .expect("Failed to execute binary");

  let stdout = String::from_utf8_lossy(&output.stdout);

  assert_eq!(stdout, "Hello, World!\n");
  assert!(output.status.success());
}

#[test]
fn test_code_signature_structure() {
  // After Tier A1 (page-rounded segments), `finish_with_signature`
  // requires the canonical segment-add sequence so it can resolve
  // segment sizes. The previous form was implicitly relying on
  // the static 256 KB caps.
  let mut macho = zo_writer_macho::MachO::new();

  macho.add_code(vec![0xC0, 0x03, 0x5F, 0xD6]);
  macho.add_pagezero_segment();
  macho.add_text_segment();
  macho.add_data_segment();
  macho.add_uuid();
  macho.add_build_version();
  macho.add_source_version();
  macho.add_main(0);
  macho.add_dyld_info();

  let binary = macho.finish_with_signature();

  // Layout is now ~32 KB (one __TEXT page + one __DATA page +
  // LINKEDIT) instead of the prior 512 KB padded form.
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
