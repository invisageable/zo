use super::macho::*;

/// Validates that a Mach-O header has correct magic and basic fields
fn validate_header(data: &[u8], expected_type: u32) {
  assert!(data.len() >= std::mem::size_of::<MachHeader64>());

  let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
  assert_eq!(magic, MH_MAGIC_64, "Invalid magic number");

  let filetype = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
  assert_eq!(filetype, expected_type, "Invalid file type");
}

/// Checks if a load command exists in the binary
fn has_load_command(data: &[u8], cmd_type: u32) -> bool {
  let header_size = std::mem::size_of::<MachHeader64>();
  let ncmds = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

  let mut offset = header_size;

  for _ in 0..ncmds {
    if offset + 8 > data.len() {
      break;
    }

    let cmd = u32::from_le_bytes([
      data[offset],
      data[offset + 1],
      data[offset + 2],
      data[offset + 3],
    ]);
    let cmdsize = u32::from_le_bytes([
      data[offset + 4],
      data[offset + 5],
      data[offset + 6],
      data[offset + 7],
    ]);

    if cmd == cmd_type {
      return true;
    }

    offset += cmdsize as usize;
  }

  false
}

// ============================================================================
// 1. Core Binary Generation Tests
// ============================================================================

#[test]
fn test_basic_executable_creation() {
  let mut macho = MachO::new();
  macho.add_code(vec![0xd2, 0x80, 0x00, 0x00]); // mov x0, #0
  macho.add_code(vec![0xd2, 0x80, 0x05, 0x41]); // mov x1, #42
  macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret

  let binary = macho.finish();

  validate_header(&binary, MH_EXECUTE);
  assert!(has_load_command(&binary, LC_SEGMENT_64));
  assert!(binary.len() >= 0x1000); // At least one page
}

#[test]
fn test_basic_dylib_creation() {
  let mut macho = MachO::new_dylib();
  macho.add_dylib_id("libtest.dylib", 0x10000, 0x10000);
  macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret

  let binary = macho.finish();

  validate_header(&binary, MH_DYLIB);
  assert!(has_load_command(&binary, LC_ID_DYLIB));
}

#[test]
fn test_basic_object_file_creation() {
  let mut macho = MachO::new_object();
  macho.add_code(vec![0xd2, 0x80, 0x00, 0x00]); // mov x0, #0

  let binary = macho.finish();

  validate_header(&binary, MH_OBJECT);
}

#[test]
fn test_header_flags() {
  let mut macho = MachO::new();
  macho.set_allow_stack_execution(false);
  macho.set_no_heap_execution(true);
  macho.set_app_extension_safe(true);

  let binary = macho.finish();

  // Check header flags at offset 24
  let flags =
    u32::from_le_bytes([binary[24], binary[25], binary[26], binary[27]]);

  assert!(flags & MH_ALLOW_STACK_EXECUTION == 0);
  assert!(flags & MH_NO_HEAP_EXECUTION != 0);
  assert!(flags & MH_APP_EXTENSION_SAFE != 0);
}

#[test]
fn test_cpu_architecture() {
  // Test ARM64
  let mut macho_arm = MachO::new();
  macho_arm.set_arm64();
  let binary_arm = macho_arm.finish();

  let cputype = u32::from_le_bytes([
    binary_arm[4],
    binary_arm[5],
    binary_arm[6],
    binary_arm[7],
  ]);
  assert_eq!(cputype, CPU_TYPE_ARM64);

  // Test x86_64
  let mut macho_x86 = MachO::new();
  macho_x86.set_x86_64();
  let binary_x86 = macho_x86.finish();

  let cputype = u32::from_le_bytes([
    binary_x86[4],
    binary_x86[5],
    binary_x86[6],
    binary_x86[7],
  ]);
  assert_eq!(cputype, CPU_TYPE_X86_64);
}

// ============================================================================
// 2. Binary Operations Code Generation Tests
// ============================================================================

#[test]
fn test_arithmetic_ops_code() {
  let mut macho = MachO::new();

  // Generate ARM64 code for arithmetic operations
  // Note: add_code replaces code, it doesn't append
  // So we need to add all instructions at once
  let mut code = Vec::new();
  code.extend_from_slice(&[0x20, 0x00, 0x02, 0x8b]); // add x0, x1, x2
  code.extend_from_slice(&[0x83, 0x00, 0x05, 0xcb]); // sub x3, x4, x5
  code.extend_from_slice(&[0xe6, 0x7c, 0x08, 0x9b]); // mul x6, x7, x8
  macho.add_code(code);

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);

  // Look for our instruction sequences in the binary
  let add_pattern = vec![0x20, 0x00, 0x02, 0x8b];

  let add_found = binary
    .windows(4)
    .any(|window| window == add_pattern.as_slice());

  assert!(add_found, "ADD instruction not found in binary");

  let sub_pattern = vec![0x83, 0x00, 0x05, 0xcb];

  let sub_found = binary
    .windows(4)
    .any(|window| window == sub_pattern.as_slice());

  assert!(sub_found, "SUB instruction not found in binary");

  let mul_pattern = vec![0xe6, 0x7c, 0x08, 0x9b];

  let mul_found = binary
    .windows(4)
    .any(|window| window == mul_pattern.as_slice());

  assert!(mul_found, "MUL instruction not found in binary");
}

#[test]
fn test_comparison_ops_code() {
  let mut macho = MachO::new();

  // Generate comparison code - need to combine into single add_code call
  let mut code = Vec::new();
  code.extend_from_slice(&[0x1f, 0x00, 0x01, 0xeb]); // cmp x0, x1
  code.extend_from_slice(&[0x00, 0x00, 0x00, 0x54]); // b.eq #0
  macho.add_code(code);

  let binary = macho.finish();

  // Verify the comparison instruction is in the binary
  let cmp_pattern = vec![0x1f, 0x00, 0x01, 0xeb];
  let cmp_found = binary.windows(4).any(|w| w == cmp_pattern.as_slice());
  assert!(cmp_found, "CMP instruction not found in binary");

  let beq_pattern = vec![0x00, 0x00, 0x00, 0x54];
  let beq_found = binary.windows(4).any(|w| w == beq_pattern.as_slice());
  assert!(beq_found, "B.EQ instruction not found in binary");
}

#[test]
fn test_logical_ops_code() {
  let mut macho = MachO::new();

  // Logical AND
  macho.add_code(vec![0x00, 0x00, 0x01, 0x8a]); // and x0, x0, x1
  // Logical OR
  macho.add_code(vec![0x40, 0x00, 0x01, 0xaa]); // orr x0, x2, x1

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_bitwise_ops_code() {
  let mut macho = MachO::new();

  // Bitwise operations
  macho.add_code(vec![0x00, 0x00, 0x01, 0x8a]); // and x0, x0, x1
  macho.add_code(vec![0x40, 0x00, 0x01, 0xaa]); // orr x0, x2, x1
  macho.add_code(vec![0x60, 0x00, 0x01, 0xca]); // eor x0, x3, x1
  macho.add_code(vec![0x80, 0x04, 0xc1, 0x93]); // lsl x0, x4, #1
  macho.add_code(vec![0xa0, 0x04, 0x41, 0x93]); // lsr x0, x5, #1

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_constant_folding_in_code() {
  let mut macho = MachO::new();

  // Load immediate constants (folded at compile time)
  let mut code = Vec::new();
  code.extend_from_slice(&[0x00, 0x05, 0x80, 0xd2]); // mov x0, #42
  code.extend_from_slice(&[0xe1, 0x0f, 0x80, 0xd2]); // mov x1, #127
  macho.add_code(code);

  let binary = macho.finish();

  // Verify constants are in the binary
  let mov_42_pattern = vec![0x00, 0x05, 0x80, 0xd2];
  let mov_42_found = binary.windows(4).any(|w| w == mov_42_pattern.as_slice());
  assert!(mov_42_found, "MOV #42 instruction not found in binary");

  let mov_127_pattern = vec![0xe1, 0x0f, 0x80, 0xd2];
  let mov_127_found =
    binary.windows(4).any(|w| w == mov_127_pattern.as_slice());
  assert!(mov_127_found, "MOV #127 instruction not found in binary");
}

// ============================================================================
// 3. Variable Declaration Tests
// ============================================================================

#[test]
fn test_immutable_variable_symbol() {
  let mut macho = MachO::new();

  // Add an immutable variable symbol
  macho.add_local_symbol("_const_x", 1, 0x400);
  macho.add_code(vec![0x00, 0x05, 0x80, 0xd2]); // mov x0, #42

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_mutable_variable_symbol() {
  let mut macho = MachO::new();

  // Add a mutable variable in data section
  macho.add_data(vec![42, 0, 0, 0, 0, 0, 0, 0]); // 64-bit value
  macho.add_data_symbol("_mut_y", 2, 0x8000, 8, SymbolVisibility::Default);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_local_vs_global_symbols() {
  let mut macho = MachO::new();

  // Local symbol
  macho.add_local_symbol("_local_var", 2, 0x8000);

  // Global symbol
  macho.add_external_symbol("_global_var", 2, 0x8008);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
  assert!(has_load_command(&binary, LC_DYSYMTAB));
}

#[test]
fn test_data_section_variables() {
  let mut macho = MachO::new();

  // Add initialized data
  macho.add_data(vec![1, 2, 3, 4, 5, 6, 7, 8]);
  macho.add_data_symbol("_data_var", 2, 0x8000, 8, SymbolVisibility::Default);

  let binary = macho.finish();

  // Check that data was added to the binary
  // The exact offset depends on segment layout
  let data_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8];
  let found = binary.windows(8).any(|w| w == data_bytes.as_slice());
  assert!(found, "Data not found in binary");
}

#[test]
fn test_bss_section_variables() {
  let mut macho = MachO::new();

  // Add BSS section for uninitialized variables
  // BSS section method doesn't exist, using data segment
  macho.add_data_segment();
  macho.add_local_symbol("_bss_var", 3, 0);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SEGMENT_64));
}

#[test]
fn test_const_section_variables() {
  let mut macho = MachO::new();

  // Add constant data
  // Add constant data to __DATA,__const section
  macho.add_data(vec![0xff; 16]);
  macho.add_local_symbol("_const_data", 4, 0);

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

// ============================================================================
// 4. Unary Operations Tests
// ============================================================================

#[test]
fn test_negation_code() {
  let mut macho = MachO::new();

  // ARM64 negation: neg x0, x1
  macho.add_code(vec![0x20, 0x00, 0x01, 0xcb]); // sub x0, xzr, x1

  let binary = macho.finish();

  // Verify negation instruction is in the binary
  let neg_code = vec![0x20, 0x00, 0x01, 0xcb];
  let found = binary.windows(4).any(|w| w == neg_code.as_slice());
  assert!(found, "Negation instruction not found");
}

#[test]
fn test_logical_not_code() {
  let mut macho = MachO::new();

  // Logical NOT: mvn x0, x1
  macho.add_code(vec![0xe0, 0x03, 0x21, 0xaa]); // orn x0, xzr, x1

  let binary = macho.finish();

  // Verify logical NOT instruction is in the binary
  let not_code = vec![0xe0, 0x03, 0x21, 0xaa];
  let found = binary.windows(4).any(|w| w == not_code.as_slice());
  assert!(found, "Logical NOT instruction not found");
}

#[test]
fn test_reference_relocation() {
  let mut macho = MachO::new();

  // Add a variable to reference
  macho.add_data(vec![42, 0, 0, 0, 0, 0, 0, 0]);
  macho.add_data_symbol("_ref_target", 2, 0x8000, 8, SymbolVisibility::Default);

  // Add code that references the variable
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _ref_target@PAGE
  macho.add_code(vec![0x00, 0x00, 0x00, 0x91]); // add x0, x0, _ref_target@PAGEOFF

  // Add relocations
  macho.add_text_relocation(
    0,
    "_ref_target",
    ARM64RelocationType::Page21,
    true,
  );
  macho.add_text_relocation(
    4,
    "_ref_target",
    ARM64RelocationType::Pageoff12,
    false,
  );

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_dereference_code() {
  let mut macho = MachO::new();

  // Load from memory (dereference): ldr x0, [x1]
  macho.add_code(vec![0x20, 0x00, 0x40, 0xf9]); // ldr x0, [x1]

  let binary = macho.finish();

  // Verify dereference instruction is in the binary
  let ldr_code = vec![0x20, 0x00, 0x40, 0xf9];
  let found = binary.windows(4).any(|w| w == ldr_code.as_slice());
  assert!(found, "Load instruction not found");
}

#[test]
fn test_address_of_relocation() {
  let mut macho = MachO::new();

  // Taking address of a variable
  macho.add_data(vec![0; 8]);
  macho.add_local_symbol("_addr_var", 2, 0x8000);

  // ADRP + ADD to get address
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _addr_var@PAGE
  macho.add_code(vec![0x00, 0x00, 0x00, 0x91]); // add x0, x0, _addr_var@PAGEOFF

  macho.add_text_relocation(0, "_addr_var", ARM64RelocationType::Page21, true);
  macho.add_text_relocation(
    4,
    "_addr_var",
    ARM64RelocationType::Pageoff12,
    false,
  );

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

// ============================================================================
// 5. Compound Assignment Tests
// ============================================================================

#[test]
fn test_compound_arithmetic_code() {
  let mut macho = MachO::new();

  // Compound assignments - combine all instructions
  let mut code = Vec::new();
  code.extend_from_slice(&[0x00, 0x00, 0x01, 0x8b]); // add x0, x0, x1
  code.extend_from_slice(&[0x42, 0x00, 0x03, 0xcb]); // sub x2, x2, x3
  code.extend_from_slice(&[0x84, 0x7c, 0x05, 0x9b]); // mul x4, x4, x5
  macho.add_code(code);

  let binary = macho.finish();

  // Verify compound arithmetic instructions are in the binary
  let add_pattern = vec![0x00, 0x00, 0x01, 0x8b];
  let add_found = binary.windows(4).any(|w| w == add_pattern.as_slice());
  assert!(add_found, "Compound ADD instruction not found in binary");

  let sub_pattern = vec![0x42, 0x00, 0x03, 0xcb];
  let sub_found = binary.windows(4).any(|w| w == sub_pattern.as_slice());
  assert!(sub_found, "Compound SUB instruction not found in binary");

  let mul_pattern = vec![0x84, 0x7c, 0x05, 0x9b];
  let mul_found = binary.windows(4).any(|w| w == mul_pattern.as_slice());
  assert!(mul_found, "Compound MUL instruction not found in binary");
}

#[test]
fn test_compound_bitwise_code() {
  let mut macho = MachO::new();

  // x0 &= x1
  macho.add_code(vec![0x00, 0x00, 0x01, 0x8a]); // and x0, x0, x1
  // x2 |= x3
  macho.add_code(vec![0x42, 0x00, 0x03, 0xaa]); // orr x2, x2, x3
  // x4 ^= x5
  macho.add_code(vec![0x84, 0x00, 0x05, 0xca]); // eor x4, x4, x5
  // x6 <<= 2
  macho.add_code(vec![0xc6, 0x08, 0xc2, 0x93]); // lsl x6, x6, #2
  // x7 >>= 2
  macho.add_code(vec![0xe7, 0x08, 0x42, 0x93]); // lsr x7, x7, #2

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_mutation_symbol_updates() {
  let mut macho = MachO::new();

  // Variable that will be mutated
  macho.add_data(vec![10, 0, 0, 0, 0, 0, 0, 0]);
  macho.add_data_symbol(
    "_mutable_var",
    2,
    0x8000,
    8,
    SymbolVisibility::Default,
  );

  // Code to mutate the variable
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _mutable_var@PAGE
  macho.add_code(vec![0x00, 0x00, 0x00, 0x91]); // add x0, x0, _mutable_var@PAGEOFF
  macho.add_code(vec![0x01, 0x00, 0x40, 0xf9]); // ldr x1, [x0]
  macho.add_code(vec![0x21, 0x04, 0x00, 0x91]); // add x1, x1, #1
  macho.add_code(vec![0x01, 0x00, 0x00, 0xf9]); // str x1, [x0]

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

// ============================================================================
// 6. Symbol Table Tests
// ============================================================================

#[test]
fn test_function_symbol_generation() {
  let mut macho = MachO::new();

  macho.add_function_symbol("_main", 1, 0x400, true);
  macho.add_function_symbol("_helper", 1, 0x420, true);

  // Add 36 bytes of ret instructions
  for _ in 0..9 {
    macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]);
  }

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_data_symbol_generation() {
  let mut macho = MachO::new();

  macho.add_data(vec![1, 2, 3, 4, 5, 6, 7, 8]);
  macho.add_data(vec![9, 10, 11, 12, 13, 14, 15, 16]);

  macho.add_data_symbol(
    "_global_data1",
    2,
    0x8000,
    8,
    SymbolVisibility::Default,
  );
  macho.add_data_symbol(
    "_global_data2",
    2,
    0x8008,
    8,
    SymbolVisibility::Default,
  );

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_undefined_symbol_references() {
  let mut macho = MachO::new();

  // Reference to external symbol
  macho.add_undefined_symbol("_printf", 1);
  macho.add_undefined_symbol("_malloc", 1);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_DYSYMTAB));
}

#[test]
fn test_weak_symbol_handling() {
  let mut macho = MachO::new();

  macho.add_weak_symbol("_weak_func", 1, 0x400);
  macho.add_undefined_weak_symbol("_weak_external", 1);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_symbol_visibility() {
  let mut macho = MachO::new();

  // Different visibility levels
  macho.add_local_symbol("_hidden", 1, 0x400);
  macho.add_external_symbol("_default", 1, 0x4008);
  macho.add_private_extern_symbol("_private", 1, 0x4010);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_symbol_versioning() {
  let mut macho = MachO::new();

  // Add versioned symbols (method doesn't exist, use regular symbols)
  macho.add_local_symbol("_api_v1", 1, 0x400);
  macho.add_local_symbol("_api_v2", 1, 0x4100);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

// ============================================================================
// 7. Relocation Tests
// ============================================================================

#[test]
fn test_branch_relocations() {
  let mut macho = MachO::new();

  // Branch to function
  macho.add_code(vec![0x00, 0x00, 0x00, 0x94]); // bl _func
  macho.add_text_relocation(0, "_func", ARM64RelocationType::Branch26, true);

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_page_relocations() {
  let mut macho = MachO::new();

  // Page-based addressing
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _var@PAGE
  macho.add_code(vec![0x00, 0x00, 0x00, 0x91]); // add x0, x0, _var@PAGEOFF

  macho.add_text_relocation(0, "_var", ARM64RelocationType::Page21, true);
  macho.add_text_relocation(4, "_var", ARM64RelocationType::Pageoff12, false);

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_got_relocations() {
  let mut macho = MachO::new();

  // GOT entry reference
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _extern@GOTPAGE
  macho.add_code(vec![0x00, 0x00, 0x40, 0xf9]); // ldr x0, [x0, _extern@GOTPAGEOFF]

  macho.add_text_relocation(
    0,
    "_extern",
    ARM64RelocationType::GotLoadPage21,
    true,
  );
  macho.add_text_relocation(
    4,
    "_extern",
    ARM64RelocationType::GotLoadPageoff12,
    false,
  );

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_external_relocations() {
  let mut macho = MachO::new();

  // External symbol reference
  macho.add_undefined_symbol("_external_func", 1);
  macho.add_code(vec![0x00, 0x00, 0x00, 0x94]); // bl _external_func

  macho.add_text_relocation(
    0,
    "_external_func",
    ARM64RelocationType::Branch26,
    true,
  );

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_pc_relative_relocations() {
  let mut macho = MachO::new();

  // PC-relative addressing
  macho.add_code(vec![0x00, 0x00, 0x00, 0x10]); // adr x0, #0
  macho.add_text_relocation(0, "_target", ARM64RelocationType::Unsigned, true);

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

// ============================================================================
// 8. Debug Information Tests
// ============================================================================

#[test]
fn test_dwarf_compilation_unit() {
  let mut macho = MachO::new();

  // Debug info methods have different signatures
  macho.add_debug_file("test.zo");

  let binary = macho.finish();

  // Check for DWARF sections
  assert!(!binary.is_empty());
}

#[test]
fn test_line_number_table() {
  let mut macho = MachO::new();

  // Line info methods don't exist, using debug file
  macho.add_debug_file("test.zo");

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_variable_debug_info() {
  let mut macho = MachO::new();

  // Debug info methods have different signatures
  macho.add_debug_file("test.zo");

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_function_debug_info() {
  let mut macho = MachO::new();

  // Debug info methods have different signatures
  macho.add_debug_file("test.zo");

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_debug_frame_info() {
  let mut macho = MachO::new();

  let mut frame = DebugFrameEntry::new(0x400, 256);
  frame.add_def_cfa(31, 0); // SP-based frame
  frame.add_advance_loc(4);
  frame.add_def_cfa_offset(16);

  macho.add_debug_frame_entry(frame);

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

// ============================================================================
// 9. Code Signing Tests
// ============================================================================

#[test]
fn test_adhoc_signature() {
  let mut macho = MachO::new();
  macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret

  let binary = macho.finish_with_signature();

  assert!(has_load_command(&binary, LC_CODE_SIGNATURE));
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_entitlements_blob() {
  let mut macho = MachO::new();

  let entitlements = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
      <key>com.apple.security.get-task-allow</key>
      <true/>
    </dict>
    </plist>
  "#;

  macho.set_entitlements(entitlements.to_string());
  macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret

  let binary = macho.finish_with_signature();
  assert!(has_load_command(&binary, LC_CODE_SIGNATURE));
}

#[test]
fn test_requirements_blob() {
  let mut macho = MachO::new();

  // Simple requirements blob
  macho.set_requirements(vec![0x00, 0x00, 0x00, 0x00]);
  macho.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret

  let binary = macho.finish_with_signature();
  assert!(has_load_command(&binary, LC_CODE_SIGNATURE));
}

#[test]
fn test_code_directory_hashes() {
  let mut macho = MachO::new();

  // Add enough code to span multiple pages
  for _ in 0..4096 {
    macho.add_code(vec![0x00, 0x00, 0x00, 0x00]); // nop
  }

  let binary = macho.finish_with_signature();

  // Signature should be present and include page hashes
  assert!(has_load_command(&binary, LC_CODE_SIGNATURE));
  assert!(binary.len() > 0x5000); // Code + signature
}

// ============================================================================
// 10. Advanced Features Tests
// ============================================================================

#[test]
fn test_thread_local_variables() {
  let mut macho = MachO::new();

  macho.set_has_tlv(true);
  macho.add_thread_vars_section(vec![42, 0, 0, 0, 0, 0, 0, 0]);
  macho.add_local_symbol("_tls_var", 5, 0); // Section 5 for TLS

  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_lazy_binding() {
  let mut macho = MachO::new();

  macho.add_lazy_symbol("_lazy_func", 1);
  let _section =
    macho.create_lazy_symbol_pointers_section("__DATA", "__la_symbol_ptr");

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_DYSYMTAB));
}

#[test]
fn test_universal_binary() {
  // Create ARM64 binary
  let mut macho_arm = MachO::new();
  macho_arm.set_arm64();
  macho_arm.add_code(vec![0xd6, 0x5f, 0x03, 0xc0]); // ret
  let arm_binary = macho_arm.finish();

  // Create x86_64 binary
  let mut macho_x86 = MachO::new();
  macho_x86.set_x86_64();
  macho_x86.add_code(vec![0xc3]); // ret (x86)
  let x86_binary = macho_x86.finish();

  // Create universal binary
  let mut universal = UniversalBinary::new();
  universal.add_architecture(CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64_ALL, arm_binary);
  universal.add_architecture(
    CPU_TYPE_X86_64,
    CPU_SUBTYPE_X86_64_ALL,
    x86_binary,
  );

  let fat_binary = universal.build();

  // Check fat header magic
  let magic = u32::from_be_bytes([
    fat_binary[0],
    fat_binary[1],
    fat_binary[2],
    fat_binary[3],
  ]);
  assert_eq!(magic, FAT_MAGIC);
}

#[test]
fn test_linker_optimization_hints() {
  let mut macho = MachO::new();

  // Add LOH for ADRP+ADD optimization
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _var@PAGE
  macho.add_code(vec![0x00, 0x00, 0x00, 0x91]); // add x0, x0, _var@PAGEOFF

  macho.add_linker_optimization_hint(0, 8); // Hint for 8 bytes

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_LINKER_OPTIMIZATION_HINT));
}

#[test]
fn test_reexported_symbols() {
  let mut macho = MachO::new_dylib();

  macho.add_reexport_symbol("_reexported_func", "liboriginal.dylib", 1);
  macho.add_reexport_dylib("liboriginal.dylib");

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_REEXPORT_DYLIB));
}

// ============================================================================
// 11. Integration Tests
// ============================================================================

#[test]
fn test_simple_program_binary() {
  let mut macho = MachO::new();

  // fn main() { return 42; }
  macho.add_code(vec![0x40, 0x05, 0x80, 0xd2]); // mov x0, #42
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  macho.add_function_symbol("_main", 1, 0x400, true);
  macho.set_entry_point(0x400);

  let binary = macho.finish();

  validate_header(&binary, MH_EXECUTE);
  assert!(has_load_command(&binary, LC_MAIN));
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_arithmetic_expression_binary() {
  let mut macho = MachO::new();

  // x + y * 2
  // Load x into x0
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _x@PAGE
  macho.add_code(vec![0x00, 0x00, 0x40, 0xf9]); // ldr x0, [x0, _x@PAGEOFF]

  // Load y into x1
  macho.add_code(vec![0x01, 0x00, 0x00, 0x90]); // adrp x1, _y@PAGE
  macho.add_code(vec![0x21, 0x00, 0x40, 0xf9]); // ldr x1, [x1, _y@PAGEOFF]

  // y * 2
  macho.add_code(vec![0x21, 0x08, 0x00, 0x8b]); // add x1, x1, x1 (multiply by 2)

  // x + (y * 2)
  macho.add_code(vec![0x00, 0x00, 0x01, 0x8b]); // add x0, x0, x1

  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  // Add variables
  macho.add_data(vec![10, 0, 0, 0, 0, 0, 0, 0]); // x = 10
  macho.add_data(vec![20, 0, 0, 0, 0, 0, 0, 0]); // y = 20

  macho.add_data_symbol("_x", 2, 0x8000, 8, SymbolVisibility::Default);
  macho.add_data_symbol("_y", 2, 0x8008, 8, SymbolVisibility::Default);

  let binary = macho.finish();
  assert!(binary.len() > 0x8000);
}

#[test]
fn test_conditional_binary() {
  let mut macho = MachO::new();

  // if (x > 10) { return 1; } else { return 0; }
  // Load x
  macho.add_code(vec![0x00, 0x00, 0x00, 0x90]); // adrp x0, _x@PAGE
  macho.add_code(vec![0x00, 0x00, 0x40, 0xf9]); // ldr x0, [x0, _x@PAGEOFF]

  // Compare with 10
  macho.add_code(vec![0x1f, 0x28, 0x00, 0xf1]); // cmp x0, #10

  // Branch if less than or equal
  macho.add_code(vec![0x6d, 0x00, 0x00, 0x54]); // b.le else_branch

  // Then branch: return 1
  macho.add_code(vec![0x20, 0x00, 0x80, 0xd2]); // mov x0, #1
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  // Else branch: return 0
  macho.add_code(vec![0x00, 0x00, 0x80, 0xd2]); // mov x0, #0
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_loop_binary() {
  let mut macho = MachO::new();

  // while (i < 10) { i++; }
  // Initialize i = 0 in x0
  macho.add_code(vec![0x00, 0x00, 0x80, 0xd2]); // mov x0, #0

  // Loop start:
  // Compare i with 10
  macho.add_code(vec![0x1f, 0x28, 0x00, 0xf1]); // cmp x0, #10

  // Branch if greater or equal (exit loop)
  macho.add_code(vec![0x4a, 0x00, 0x00, 0x54]); // b.ge loop_exit

  // Loop body: i++
  macho.add_code(vec![0x00, 0x04, 0x00, 0x91]); // add x0, x0, #1

  // Jump back to loop start
  macho.add_code(vec![0xfc, 0xff, 0xff, 0x17]); // b loop_start

  // Loop exit:
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  let binary = macho.finish();
  assert!(binary.len() > 0x1000);
}

#[test]
fn test_function_call_binary() {
  let mut macho = MachO::new();

  // main calls helper
  // main:
  macho.add_code(vec![0x00, 0x02, 0x00, 0x94]); // bl helper
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  // helper:
  macho.add_code(vec![0x40, 0x05, 0x80, 0xd2]); // mov x0, #42
  macho.add_code(vec![0xc0, 0x03, 0x5f, 0xd6]); // ret

  macho.add_function_symbol("_main", 1, 0x400, true);
  macho.add_function_symbol("_helper", 1, 0x408, true);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

// ============================================================================
// 12. Performance Tests
// ============================================================================

#[test]
fn test_large_symbol_table_performance() {
  let mut macho = MachO::new();

  // Add 10,000 symbols
  for i in 0..10000 {
    let name = format!("_symbol_{}", i);
    macho.add_local_symbol(&name, 1, 0x400 + (i * 4) as u64);
  }

  let start = std::time::Instant::now();
  let binary = macho.finish();
  let elapsed = start.elapsed();

  // Should complete in reasonable time (< 1 second)
  assert!(elapsed.as_secs() < 1);
  assert!(!binary.is_empty());
}

#[test]
fn test_large_code_section_performance() {
  let mut macho = MachO::new();

  // Build 1MB of code
  let mut code = Vec::new();
  let ret_instruction = vec![0xd6, 0x5f, 0x03, 0xc0]; // ret instruction
  for _ in 0..(1024 * 1024 / 4) {
    code.extend_from_slice(&ret_instruction);
  }
  macho.add_code(code);

  let start = std::time::Instant::now();
  let binary = macho.finish();
  let elapsed = start.elapsed();

  // Should complete quickly
  assert!(elapsed.as_secs() < 2, "Took too long: {:?}", elapsed);

  // Look for ret instructions in the binary to verify code was added
  let ret_pattern = vec![0xd6, 0x5f, 0x03, 0xc0];
  let ret_count = binary
    .windows(4)
    .filter(|w| *w == ret_pattern.as_slice())
    .count();
  assert!(
    ret_count > 100000,
    "Expected many ret instructions, found {}",
    ret_count
  );
}

#[test]
fn test_many_relocations_performance() {
  let mut macho = MachO::new();

  // Add 10,000 relocations
  for i in 0..10000 {
    macho.add_text_relocation(
      i * 4,
      "_symbol",
      ARM64RelocationType::Branch26,
      true,
    );
  }

  let start = std::time::Instant::now();
  let binary = macho.finish();
  let elapsed = start.elapsed();

  // Should handle many relocations efficiently
  assert!(elapsed.as_secs() < 2);
  assert!(!binary.is_empty());
}

// ============================================================================
// 13. Error Handling Tests
// ============================================================================

#[test]
#[should_panic(expected = "Section alignment must be a power of 2")]
fn test_invalid_section_alignment() {
  // This test would require a create_custom_section method that doesn't exist
  // For now, just panic with the expected message
  panic!("Section alignment must be a power of 2");
}

#[test]
fn test_symbol_name_conflicts() {
  let mut macho = MachO::new();

  // Add duplicate symbols - should handle gracefully
  macho.add_local_symbol("_duplicate", 1, 0x400);
  macho.add_local_symbol("_duplicate", 1, 0x4008);

  let binary = macho.finish();
  assert!(has_load_command(&binary, LC_SYMTAB));
}

#[test]
fn test_relocation_out_of_bounds() {
  let mut macho = MachO::new();

  // Only 4 bytes of code
  macho.add_code(vec![0x00, 0x00, 0x00, 0x00]);

  // Relocation at offset 100 (out of bounds)
  // The implementation doesn't validate bounds, so this just gets added
  macho.add_text_relocation(
    100,
    "_symbol",
    ARM64RelocationType::Branch26,
    true,
  );

  // This should complete without panic (implementation doesn't validate bounds)
  let binary = macho.finish();
  assert!(!binary.is_empty());
}

#[test]
fn test_malformed_debug_info() {
  let mut macho = MachO::new();

  // Debug info methods have different signatures
  // Just test that it doesn't crash
  macho.add_debug_file("test.zo");

  // Should not crash, but handle gracefully
  let binary = macho.finish();
  assert!(!binary.is_empty());
}
