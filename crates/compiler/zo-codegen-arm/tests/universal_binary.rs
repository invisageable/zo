use zo_codegen_arm::ARM64Gen;
use zo_writer_macho::UniversalBinary;

use std::process::Command;

#[test]
fn test_universal_binary_creation() {
  // Create ARM64 binary
  let arm64_binary = ARM64Gen::generate_hello_world_signed();

  // For now, we'll use the same binary for x86_64 (in production, you'd
  // generate a real x86_64 binary) This is just to test the universal binary
  // structure
  let x86_64_binary = arm64_binary.clone();

  // Create universal binary
  let universal =
    UniversalBinary::create_universal(arm64_binary, x86_64_binary);

  // Verify the binary starts with the fat magic number
  assert_eq!(&universal[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);

  // Verify we have 2 architectures
  assert_eq!(
    u32::from_be_bytes([
      universal[4],
      universal[5],
      universal[6],
      universal[7]
    ]),
    2
  );

  // Write to file
  let path = "tests/hello_universal";
  ARM64Gen::write_executable(universal, path)
    .expect("Failed to write universal binary");

  // Use 'file' command to verify it's recognized as a universal binary
  let output = Command::new("file")
    .arg(path)
    .output()
    .expect("Failed to run 'file' command");

  let file_output = String::from_utf8_lossy(&output.stdout);
  assert!(
    file_output.contains("Mach-O universal binary")
      || file_output.contains("fat file"),
    "File command didn't recognize universal binary: {}",
    file_output
  );

  // Clean up
  std::fs::remove_file(path).ok();
}

#[test]
fn test_universal_binary_64bit() {
  // Create ARM64 binary
  let arm64_binary = ARM64Gen::generate_hello_world_signed();

  // Create a 64-bit universal binary
  let mut builder = UniversalBinary::new_64bit();
  builder.add_arm64(arm64_binary);
  let universal = builder.build();

  // Verify the binary starts with the 64-bit fat magic number
  assert_eq!(&universal[0..4], &[0xCA, 0xFE, 0xBA, 0xBF]);

  // Verify we have 1 architecture
  assert_eq!(
    u32::from_be_bytes([
      universal[4],
      universal[5],
      universal[6],
      universal[7]
    ]),
    1
  );
}

#[test]
fn test_universal_binary_alignment() {
  // Create two small test binaries
  let binary1 = vec![0x00, 0x01, 0x02, 0x03];
  let binary2 = vec![0x10, 0x11, 0x12, 0x13];

  let mut builder = UniversalBinary::new();
  builder.add_architecture(0x01000007, 0x00000003, binary1);
  builder.add_architecture(0x0100000c, 0x00000000, binary2);
  let universal = builder.build();

  // Parse the fat header
  let nfat_arch = u32::from_be_bytes([
    universal[4],
    universal[5],
    universal[6],
    universal[7],
  ]);
  assert_eq!(nfat_arch, 2);

  // Check first architecture descriptor (starts at offset 8, after FatHeader)
  // FatArch struct: cputype(4) + cpusubtype(4) + offset(4) + size(4) + align(4)
  // = 20 bytes
  let arch1_offset = u32::from_be_bytes([
    universal[8 + 8],
    universal[8 + 9],
    universal[8 + 10],
    universal[8 + 11],
  ]);
  let arch1_align = u32::from_be_bytes([
    universal[8 + 16],
    universal[8 + 17],
    universal[8 + 18],
    universal[8 + 19],
  ]);

  // Verify alignment is page-aligned (4KB = 2^12)
  assert_eq!(arch1_align, 12);
  assert_eq!(
    arch1_offset % 4096,
    0,
    "First architecture should be page-aligned"
  );

  // Check second architecture descriptor (starts at offset 8 + 20 = 28)
  let arch2_offset = u32::from_be_bytes([
    universal[28 + 8],
    universal[28 + 9],
    universal[28 + 10],
    universal[28 + 11],
  ]);
  let arch2_align = u32::from_be_bytes([
    universal[28 + 16],
    universal[28 + 17],
    universal[28 + 18],
    universal[28 + 19],
  ]);

  assert_eq!(arch2_align, 12);
  assert_eq!(
    arch2_offset % 4096,
    0,
    "Second architecture should be page-aligned"
  );

  // Verify architectures don't overlap
  assert!(arch2_offset > arch1_offset);
}
