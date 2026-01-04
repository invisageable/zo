use zo_writer_macho::{MachO, SymbolVisibility};

#[test]
fn test_external_function_call() {
  // Create a simple program that calls an external function
  let mut macho = MachO::new();

  // Add a simple function that calls printf
  let code = vec![
    // Save frame pointer and link register
    0xFD, 0x7B, 0xBF, 0xA9, // stp x29, x30, [sp, #-16]!
    0xFD, 0x03, 0x00, 0x91, // mov x29, sp
    // Load address of format string (will be relocated)
    0x00, 0x00, 0x00, 0x90, // adrp x0, 0 (page of string)
    0x00, 0x00, 0x00, 0x91, // add x0, x0, 0 (offset in page)
    // Call printf (will be relocated)
    0x00, 0x00, 0x00, 0x94, // bl _printf
    // Return 0
    0x00, 0x00, 0x80, 0xD2, // mov x0, #0
    // Restore and return
    0xFD, 0x7B, 0xC1, 0xA8, // ldp x29, x30, [sp], #16
    0xC0, 0x03, 0x5F, 0xD6, // ret
  ];

  macho.add_code(code);

  // Add format string in data section
  let hello_str = b"Hello from relocations!\n\0";
  macho.add_data(hello_str.to_vec());

  // Add relocations for the string reference
  // ADRP at offset 8
  macho.add_page_relocation(8, 12, "hello_string");

  // Add relocation for printf call
  // BL at offset 16
  macho.add_branch_relocation(16, "_printf");

  // Add printf as undefined external symbol
  macho.add_undefined_symbol("_printf", 1); // libSystem ordinal

  // Add the string symbol
  macho.add_data_symbol(
    "hello_string",
    0,
    hello_str.len() as u64,
    0,
    SymbolVisibility::Default,
  );

  // Add segments
  macho.add_pagezero_segment();
  macho.add_text_segment();
  macho.add_data_segment();

  // Add load commands
  macho.add_dylinker();
  macho.add_dylib("/usr/lib/libSystem.B.dylib");
  macho.add_uuid();
  macho.add_build_version();
  macho.add_source_version();
  macho.add_main(0x400); // Entry at __text start
  macho.add_dyld_info();

  // Build the binary
  let binary = macho.finish();

  // Basic validation
  assert!(!binary.is_empty());
  assert_eq!(&binary[0..4], &[0xCF, 0xFA, 0xED, 0xFE]); // MH_MAGIC_64
}

#[test]
fn test_got_relocation() {
  let mut macho = MachO::new();

  // Code that uses GOT for external symbol
  let code = vec![
    // Load global variable address via GOT
    0x00, 0x00, 0x00, 0x90, // adrp x0, _global_var@GOTPAGE
    0x00, 0x00, 0x40, 0xF9, // ldr x0, [x0, _global_var@GOTPAGEOFF]
    // Load value
    0x00, 0x00, 0x40, 0xB9, // ldr w0, [x0]
    // Return
    0xC0, 0x03, 0x5F, 0xD6, // ret
  ];

  macho.add_code(code);

  // Add GOT relocations
  macho.add_got_relocation(0, 4, "_global_var");

  // Add undefined symbol
  macho.add_undefined_symbol("_global_var", 1);

  // Add segments
  macho.add_pagezero_segment();
  macho.add_text_segment();

  // Add load commands
  macho.add_dylinker();
  macho.add_dylib("/usr/lib/libSystem.B.dylib");
  macho.add_main(0x400);

  let binary = macho.finish();

  // Validate
  assert!(!binary.is_empty());

  // Check CPU type is ARM64
  let cputype =
    u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);
  assert_eq!(cputype, 0x0100000C); // CPU_TYPE_ARM64
}
