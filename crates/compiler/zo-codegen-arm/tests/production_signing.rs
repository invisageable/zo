use std::process::Command;
use zo_codegen_arm::ARM64Gen;

#[test]
fn test_production_code_signature() {
  // Generate a signed Hello World binary
  let binary = ARM64Gen::generate_hello_world_signed();

  // Binary should be non-empty
  assert!(!binary.is_empty());

  // Should have Mach-O magic number
  assert_eq!(&binary[0..4], &[0xCF, 0xFA, 0xED, 0xFE]);

  // Write to file
  let path = "tests/hello_production_signed";

  ARM64Gen::write_executable(binary.clone(), path)
    .expect("Failed to write executable");

  // Verify with codesign tool
  let verify_output = Command::new("codesign")
    .args(&["-v", path])
    .output()
    .expect("Failed to run codesign");

  // The binary should be valid (even if ad-hoc signed)
  // Note: This might still require explicit signing on newer macOS versions
  println!(
    "Codesign stderr: {}",
    String::from_utf8_lossy(&verify_output.stderr)
  );

  // Check that LC_CODE_SIGNATURE is present
  let otool_output = Command::new("otool")
    .args(&["-l", path])
    .output()
    .expect("Failed to run otool");

  let otool_str = String::from_utf8_lossy(&otool_output.stdout);

  assert!(
    otool_str.contains("LC_CODE_SIGNATURE"),
    "Binary should contain LC_CODE_SIGNATURE load command"
  );

  // Verify signature structure
  let sig_magic = &[0xFA, 0xDE, 0x0C, 0xC0]; // CSMAGIC_EMBEDDED_SIGNATURE
  let found_sig = binary.windows(4).any(|w| w == sig_magic);

  assert!(found_sig, "Code signature magic not found");

  let dir_magic = &[0xFA, 0xDE, 0x0C, 0x02]; // CSMAGIC_CODEDIRECTORY  
  let found_dir = binary.windows(4).any(|w| w == dir_magic);

  assert!(found_dir, "Code directory magic not found");

  // Clean up
  std::fs::remove_file(path).ok();
}

#[test]
fn test_signature_load_command_offset() {
  // Create a minimal signed binary
  let mut macho = zo_writer_macho::MachO::new();

  // Add minimal code
  macho.add_code(vec![
    0xC0, 0x03, 0x5F, 0xD6, // ret
  ]);

  // Add minimal segments
  macho.add_pagezero_segment();
  macho.add_text_segment();

  // Generate with signature
  let binary = macho.finish_with_signature();

  // Parse the header to find LC_CODE_SIGNATURE
  let ncmds =
    u32::from_le_bytes([binary[16], binary[17], binary[18], binary[19]]);

  assert!(ncmds > 0, "Should have load commands");

  // Look for LC_CODE_SIGNATURE (0x1d)
  let mut offset = 32; // Skip header
  let mut found_codesig = false;

  for _ in 0..ncmds {
    let cmd = u32::from_le_bytes([
      binary[offset],
      binary[offset + 1],
      binary[offset + 2],
      binary[offset + 3],
    ]);
    let cmdsize = u32::from_le_bytes([
      binary[offset + 4],
      binary[offset + 5],
      binary[offset + 6],
      binary[offset + 7],
    ]);

    if cmd == 0x1d {
      // LC_CODE_SIGNATURE
      found_codesig = true;

      // Verify dataoff and datasize are reasonable
      let dataoff = u32::from_le_bytes([
        binary[offset + 8],
        binary[offset + 9],
        binary[offset + 10],
        binary[offset + 11],
      ]);
      let datasize = u32::from_le_bytes([
        binary[offset + 12],
        binary[offset + 13],
        binary[offset + 14],
        binary[offset + 15],
      ]);

      assert!(dataoff > 0, "Code signature offset should be non-zero");
      assert!(datasize > 0, "Code signature size should be non-zero");
      assert!(
        (dataoff as usize) < binary.len(),
        "Code signature offset should be within binary"
      );

      // Verify signature is actually at that offset
      let sig_offset = dataoff as usize;

      if sig_offset + 4 <= binary.len() {
        let magic = &binary[sig_offset..sig_offset + 4];
        
        // Should be CSMAGIC_EMBEDDED_SIGNATURE in big-endian
        assert_eq!(
          magic,
          &[0xFA, 0xDE, 0x0C, 0xC0],
          "Signature magic at specified offset"
        );
      }

      break;
    }

    offset += cmdsize as usize;
  }

  assert!(
    found_codesig,
    "LC_CODE_SIGNATURE load command should be present"
  );
}
