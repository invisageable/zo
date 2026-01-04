//! Mach-O file format writer for ARM64 executables
//!
//! This module provides functionality to generate Mach-O executables for macOS
//! ARM64. It creates the necessary file structure including headers, load
//! commands, and segments required for a valid executable that can run on Apple
//! Silicon Macs.
//!
//! Based on Graydon Hoare's OCaml implementation from rust-prehistory.
//!
//! # Example
//! ```ignore
//! let mut macho = MachO::new();
//! macho.add_code(vec![/* ARM64 machine code */]);
//! macho.add_pagezero_segment();
//! macho.add_text_segment();
//! let binary = macho.finish();
//! ```
//!
//! references:
//!
//! https://en.wikipedia.org/wiki/Mach-O

use rustc_hash::FxHashMap as HashMap;
use sha2::{Digest, Sha256};

// Mach-O constants - ported from Graydon's OCaml implementation

// Fat binary constants
pub(crate) const FAT_MAGIC: u32 = 0xcafebabe; // Fat binary magic number
pub(crate) const FAT_MAGIC_64: u32 = 0xcafebabf; // 64-bit fat binary magic number
// Note: FAT_CIGAM variants are for reading fat binaries, which we don't
// implement

// Memory layout constants
pub(crate) const PAGE_SIZE: u32 = 0x1000; // 4KB page size
pub(crate) const CODE_OFFSET: u32 = 0x400; // Code starts at 1KB after header
pub(crate) const TEXT_SEGMENT_SIZE: u32 = PAGE_SIZE * 4; // 16KB for TEXT segment (4 pages)
pub(crate) const DATA_SEGMENT_SIZE: u32 = PAGE_SIZE * 4; // 16KB for DATA segment (4 pages)

// Virtual memory addresses
pub(crate) const VM_BASE: u64 = 0x100000000; // Base VM address for 64-bit executables
pub(crate) const TEXT_VM_ADDR: u64 = VM_BASE;
pub(crate) const DATA_VM_ADDR: u64 = VM_BASE + TEXT_SEGMENT_SIZE as u64;

pub(crate) const LINKEDIT_VM_ADDR: u64 =
  VM_BASE + (TEXT_SEGMENT_SIZE + DATA_SEGMENT_SIZE) as u64;

// File offsets
pub(crate) const TEXT_FILE_OFFSET: u64 = 0; // TEXT includes the header
pub(crate) const DATA_FILE_OFFSET: u64 = TEXT_SEGMENT_SIZE as u64; // DATA at next page boundary

pub(crate) const LINKEDIT_FILE_OFFSET: u64 =
  (TEXT_SEGMENT_SIZE + DATA_SEGMENT_SIZE) as u64; // LINKEDIT after DATA

// Alignment constants
pub(crate) const ALIGNMENT_8BYTE_MASK: usize = !7; // Mask for 8-byte alignment
pub(crate) const SECTION_ALIGN_4BYTE: u32 = 2; // 2^2 = 4 bytes alignment

// Version constants
pub(crate) const MACOS_VERSION_14_0: u32 = 0x000E0000; // macOS 14.0

// Magic numbers
pub(crate) const MH_MAGIC_64: u32 = 0xFEEDFACF; // 64-bit Mach-O

// CPU types
pub(crate) const CPU_ARCH_ABI64: u32 = 0x01000000;
pub(crate) const CPU_TYPE_X86: u32 = 7;
pub(crate) const CPU_TYPE_X86_64: u32 = CPU_TYPE_X86 | CPU_ARCH_ABI64;
pub(crate) const CPU_TYPE_ARM: u32 = 12;
pub(crate) const CPU_TYPE_ARM64: u32 = CPU_TYPE_ARM | CPU_ARCH_ABI64;

// CPU subtypes
pub(crate) const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
pub(crate) const CPU_SUBTYPE_ARM64E: u32 = 2;
pub(crate) const CPU_SUBTYPE_X86_64_ALL: u32 = 3;

// File types
pub(crate) const MH_OBJECT: u32 = 0x1; // Relocatable object file
pub(crate) const MH_EXECUTE: u32 = 0x2; // Demand paged executable
pub(crate) const MH_DYLIB: u32 = 0x6; // Dynamically bound shared library
pub(crate) const MH_BUNDLE: u32 = 0x8; // Dynamically bound bundle
pub(crate) const MH_KEXT_BUNDLE: u32 = 0xb; // Kernel extension

// File flags
pub(crate) const MH_NOUNDEFS: u32 = 0x1; // No undefined references
pub(crate) const MH_DYLDLINK: u32 = 0x4; // Input for dynamic linker
pub(crate) const MH_TWOLEVEL: u32 = 0x80; // Two-level namespace bindings
pub(crate) const MH_SUBSECTIONS_VIA_SYMBOLS: u32 = 0x2000; // Safe to divide into sub-sections
pub(crate) const MH_WEAK_DEFINES: u32 = 0x8000; // Has weak defined symbols
pub(crate) const MH_BINDS_TO_WEAK: u32 = 0x10000; // Uses weak symbols
pub(crate) const MH_ALLOW_STACK_EXECUTION: u32 = 0x20000; // Allow stack execution
pub(crate) const MH_ROOT_SAFE: u32 = 0x40000; // Safe for root execution
pub(crate) const MH_SETUID_SAFE: u32 = 0x80000; // Safe for setuid execution
pub(crate) const MH_NO_REEXPORTED_DYLIBS: u32 = 0x100000; // No re-exported dylibs
pub(crate) const MH_PIE: u32 = 0x200000; // Position independent executable
pub(crate) const MH_HAS_TLV_DESCRIPTORS: u32 = 0x800000; // Has thread local variables
pub(crate) const MH_NO_HEAP_EXECUTION: u32 = 0x1000000; // No heap execution
pub(crate) const MH_APP_EXTENSION_SAFE: u32 = 0x02000000; // App extension safe

// Load commands
pub(crate) const LC_SYMTAB: u32 = 0x2; // Symbol table
pub(crate) const LC_DYSYMTAB: u32 = 0xb; // Dynamic symbol table
pub(crate) const LC_LOAD_DYLIB: u32 = 0xc; // Load dynamic library
pub(crate) const LC_ID_DYLIB: u32 = 0xd; // Dynamic library identification
pub(crate) const LC_LOAD_DYLINKER: u32 = 0xe; // Load dynamic linker
pub(crate) const LC_ID_DYLINKER: u32 = 0xf; // Dynamic linker identification
pub(crate) const LC_SUB_FRAMEWORK: u32 = 0x12; // Sub framework
pub(crate) const LC_SUB_UMBRELLA: u32 = 0x13; // Sub umbrella
pub(crate) const LC_SUB_CLIENT: u32 = 0x14; // Sub client
pub(crate) const LC_SUB_LIBRARY: u32 = 0x15; // Sub library
pub(crate) const LC_LOAD_WEAK_DYLIB: u32 = 0x80000018; // Load weak dynamic library
pub(crate) const LC_SEGMENT_64: u32 = 0x19; // 64-bit segment
pub(crate) const LC_UUID: u32 = 0x1b; // UUID
pub(crate) const LC_RPATH: u32 = 0x8000001c; // Runtime path
pub(crate) const LC_CODE_SIGNATURE: u32 = 0x1d; // Code signature
pub(crate) const LC_REEXPORT_DYLIB: u32 = 0x8000001f; // Re-export dynamic library
pub(crate) const LC_LAZY_LOAD_DYLIB: u32 = 0x20; // Lazy load dynamic library
pub(crate) const LC_DYLD_INFO_ONLY: u32 = 0x80000022; // Compressed dyld info (only)
pub(crate) const LC_LOAD_UPWARD_DYLIB: u32 = 0x80000023; // Load upward dynamic library
pub(crate) const LC_VERSION_MIN_MACOSX: u32 = 0x24; // Minimum macOS version
pub(crate) const LC_VERSION_MIN_IPHONEOS: u32 = 0x25; // Minimum iOS version
pub(crate) const LC_FUNCTION_STARTS: u32 = 0x26; // Compressed function start addresses
pub(crate) const LC_MAIN: u32 = 0x80000028; // Main entry point
pub(crate) const LC_DATA_IN_CODE: u32 = 0x29; // Data in code entries
pub(crate) const LC_SOURCE_VERSION: u32 = 0x2A; // Source version
pub(crate) const LC_ENCRYPTION_INFO_64: u32 = 0x2C; // 64-bit encrypted segment info
pub(crate) const LC_LINKER_OPTION: u32 = 0x2D; // Linker options
pub(crate) const LC_LINKER_OPTIMIZATION_HINT: u32 = 0x2E; // Optimization hints
pub(crate) const LC_VERSION_MIN_TVOS: u32 = 0x2F; // Minimum tvOS version
pub(crate) const LC_VERSION_MIN_WATCHOS: u32 = 0x30; // Minimum watchOS version
pub(crate) const LC_BUILD_VERSION: u32 = 0x32; // Build version
pub(crate) const LC_DYLD_EXPORTS_TRIE: u32 = 0x80000033; // Export trie
pub(crate) const LC_DYLD_CHAINED_FIXUPS: u32 = 0x80000034; // Chained fixups

// Code signature constants
pub(crate) const CSMAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade0cc0;
pub(crate) const CSMAGIC_CODEDIRECTORY: u32 = 0xfade0c02;
pub(crate) const CSMAGIC_REQUIREMENTS: u32 = 0xfade0c01;
pub(crate) const CSMAGIC_ENTITLEMENTS: u32 = 0xfade7171;

// Code signature flags
pub(crate) const CS_ADHOC: u32 = 0x2;
pub(crate) const CS_LINKER_SIGNED: u32 = 0x20000;

// Hash types
pub(crate) const CS_HASHTYPE_SHA256: u8 = 2;
pub(crate) const CS_HASH_SIZE_SHA256: usize = 32;

// VM protection flags
pub(crate) const VM_PROT_NONE: u32 = 0x0;
pub(crate) const VM_PROT_READ: u32 = 0x1;
pub(crate) const VM_PROT_WRITE: u32 = 0x2;
pub(crate) const VM_PROT_EXECUTE: u32 = 0x4;

// Section types
pub(crate) const S_REGULAR: u32 = 0x0; // Regular section
pub(crate) const S_ZEROFILL: u32 = 0x1; // Zero fill on demand
pub(crate) const S_CSTRING_LITERALS: u32 = 0x2; // C string literals
pub(crate) const S_4BYTE_LITERALS: u32 = 0x3; // 4 byte literal pool
pub(crate) const S_8BYTE_LITERALS: u32 = 0x4; // 8 byte literal pool
pub(crate) const S_LITERAL_POINTERS: u32 = 0x5; // Pointers to literals
pub(crate) const S_NON_LAZY_SYMBOL_POINTERS: u32 = 0x6; // Non-lazy symbol pointers
pub(crate) const S_LAZY_SYMBOL_POINTERS: u32 = 0x7; // Lazy symbol pointers
pub(crate) const S_SYMBOL_STUBS: u32 = 0x8; // Symbol stubs
pub(crate) const S_MOD_INIT_FUNC_POINTERS: u32 = 0x9; // Module init func pointers
pub(crate) const S_MOD_TERM_FUNC_POINTERS: u32 = 0xa; // Module term func pointers
pub(crate) const S_COALESCED: u32 = 0xb; // Coalesced symbols
pub(crate) const S_GB_ZEROFILL: u32 = 0xc; // Zero fill (>= 4GB)
pub(crate) const S_INTERPOSING: u32 = 0xd; // Interposing section
pub(crate) const S_16BYTE_LITERALS: u32 = 0xe; // 16 byte literal pool
pub(crate) const S_DTRACE_DOF: u32 = 0xf; // DTrace DOF
pub(crate) const S_LAZY_DYLIB_SYMBOL_POINTERS: u32 = 0x10; // Lazy dylib symbol pointers
pub(crate) const S_THREAD_LOCAL_REGULAR: u32 = 0x11; // Thread local regular
pub(crate) const S_THREAD_LOCAL_ZEROFILL: u32 = 0x12; // Thread local zerofill
pub(crate) const S_THREAD_LOCAL_VARIABLES: u32 = 0x13; // Thread local variables
pub(crate) const S_THREAD_LOCAL_VARIABLE_POINTERS: u32 = 0x14; // Thread local variable pointers
pub(crate) const S_THREAD_LOCAL_INIT_FUNCTION_POINTERS: u32 = 0x15; // Thread local init func ptrs

// Section attributes
pub(crate) const S_ATTR_PURE_INSTRUCTIONS: u32 = 0x80000000; // Pure instructions
pub(crate) const S_ATTR_NO_DEAD_STRIP: u32 = 0x10000000; // No dead stripping
pub(crate) const S_ATTR_LIVE_SUPPORT: u32 = 0x08000000; // Live support
pub(crate) const S_ATTR_SELF_MODIFYING_CODE: u32 = 0x04000000; // Self modifying code
pub(crate) const S_ATTR_DEBUG: u32 = 0x02000000; // Debug section
pub(crate) const S_ATTR_SOME_INSTRUCTIONS: u32 = 0x00000400; // Some instructions

// Symbol table entry types and flags
pub(crate) const N_EXT: u8 = 0x01; // External symbol
pub(crate) const N_PEXT: u8 = 0x10; // Private external symbol
pub(crate) const N_TYPE: u8 = 0x0e; // Mask for type bits
pub(crate) const N_STAB: u8 = 0xe0; // Mask for debugger symbol table entries

// Symbol types (values for N_TYPE bits)
pub(crate) const N_UNDF: u8 = 0x0; // Undefined symbol
pub(crate) const N_ABS: u8 = 0x2; // Absolute symbol
pub(crate) const N_SECT: u8 = 0xe; // Defined in section
pub(crate) const N_PBUD: u8 = 0xc; // Prebound undefined (defined in dylib)
pub(crate) const N_INDR: u8 = 0xa; // Indirect symbol

// Debug symbol types (STAB)
pub(crate) const N_GSYM: u8 = 0x20; // Global symbol
pub(crate) const N_FNAME: u8 = 0x22; // Function name
pub(crate) const N_FUN: u8 = 0x24; // Function
pub(crate) const N_STSYM: u8 = 0x26; // Static symbol
pub(crate) const N_LCSYM: u8 = 0x28; // .lcomm symbol
pub(crate) const N_BNSYM: u8 = 0x2e; // Begin nsect symbol
pub(crate) const N_OPT: u8 = 0x3c; // Debugger options
pub(crate) const N_RSYM: u8 = 0x40; // Register symbol
pub(crate) const N_SLINE: u8 = 0x44; // Source line
pub(crate) const N_ENSYM: u8 = 0x4e; // End nsect symbol
pub(crate) const N_SSYM: u8 = 0x60; // Structure symbol
pub(crate) const N_SO: u8 = 0x64; // Source file name
pub(crate) const N_OSO: u8 = 0x66; // Object file name
pub(crate) const N_LSYM: u8 = 0x80; // Local symbol
pub(crate) const N_BINCL: u8 = 0x82; // Begin include file
pub(crate) const N_SOL: u8 = 0x84; // Included source file name
pub(crate) const N_PARAMS: u8 = 0x86; // Compiler parameters
pub(crate) const N_VERSION: u8 = 0x88; // Compiler version
pub(crate) const N_OLEVEL: u8 = 0x8A; // Compiler optimization level
pub(crate) const N_PSYM: u8 = 0xa0; // Parameter symbol
pub(crate) const N_EINCL: u8 = 0xa2; // End include file
pub(crate) const N_ENTRY: u8 = 0xa4; // Alternate entry point
pub(crate) const N_LBRAC: u8 = 0xc0; // Left bracket
pub(crate) const N_EXCL: u8 = 0xc2; // Excluded include file
pub(crate) const N_RBRAC: u8 = 0xe0; // Right bracket
pub(crate) const N_BCOMM: u8 = 0xe2; // Begin common
pub(crate) const N_ECOMM: u8 = 0xe4; // End common
pub(crate) const N_ECOML: u8 = 0xe8; // End common (local name)
pub(crate) const N_LENG: u8 = 0xfe; // Length of preceding entry

// Reference type flags for n_desc (low 4 bits)
pub(crate) const REFERENCE_FLAG_UNDEFINED_NON_LAZY: u16 = 0x0;
pub(crate) const REFERENCE_FLAG_UNDEFINED_LAZY: u16 = 0x1;
pub(crate) const REFERENCE_FLAG_DEFINED: u16 = 0x2;
pub(crate) const REFERENCE_FLAG_PRIVATE_DEFINED: u16 = 0x3;
pub(crate) const REFERENCE_FLAG_PRIVATE_UNDEFINED_NON_LAZY: u16 = 0x4;
pub(crate) const REFERENCE_FLAG_PRIVATE_UNDEFINED_LAZY: u16 = 0x5;

// Symbol description flags (n_desc high bits)
pub(crate) const N_WEAK_REF: u16 = 0x0040; // Symbol is a weak reference
pub(crate) const N_WEAK_DEF: u16 = 0x0080; // Symbol is a weak definition
pub(crate) const N_REF_TO_WEAK: u16 = 0x0080; // Reference to a weak symbol
pub(crate) const N_ARM_THUMB_DEF: u16 = 0x0008; // ARM Thumb function
pub(crate) const N_SYMBOL_RESOLVER: u16 = 0x0100; // Symbol is a resolver function
pub(crate) const N_NO_DEAD_STRIP: u16 = 0x0020; // Don't dead strip symbol

// Build platform
pub(crate) const PLATFORM_MACOS: u32 = 1;

// DWARF constants
pub(crate) const DW_TAG_COMPILE_UNIT: u16 = 0x11;
pub(crate) const DW_TAG_SUBPROGRAM: u16 = 0x2e;
pub(crate) const DW_TAG_VARIABLE: u16 = 0x34;

// DWARF children constants
pub(crate) const DW_CHILDREN_NO: u8 = 0x00;
pub(crate) const DW_CHILDREN_YES: u8 = 0x01;
pub(crate) const DW_TAG_BASE_TYPE: u16 = 0x24;
pub(crate) const DW_TAG_POINTER_TYPE: u16 = 0x0f;
pub(crate) const DW_TAG_STRUCTURE_TYPE: u16 = 0x13;
pub(crate) const DW_TAG_MEMBER: u16 = 0x0d;
pub(crate) const DW_TAG_ARRAY_TYPE: u16 = 0x01;
pub(crate) const DW_TAG_SUBRANGE_TYPE: u16 = 0x21;

pub(crate) const DW_AT_NAME: u16 = 0x03;
pub(crate) const DW_AT_STMT_LIST: u16 = 0x10;
pub(crate) const DW_AT_LOW_PC: u16 = 0x11;
pub(crate) const DW_AT_HIGH_PC: u16 = 0x12;
pub(crate) const DW_AT_LANGUAGE: u16 = 0x13;
pub(crate) const DW_AT_PRODUCER: u16 = 0x25;
pub(crate) const DW_AT_COMP_DIR: u16 = 0x1b;
pub(crate) const DW_AT_TYPE: u16 = 0x49;
pub(crate) const DW_AT_LOCATION: u16 = 0x02;
pub(crate) const DW_AT_BYTE_SIZE: u16 = 0x0b;
pub(crate) const DW_AT_ENCODING: u16 = 0x3e;
pub(crate) const DW_AT_DECL_FILE: u16 = 0x3a;
pub(crate) const DW_AT_DECL_LINE: u16 = 0x3b;
pub(crate) const DW_AT_EXTERNAL: u16 = 0x3f;
pub(crate) const DW_AT_FRAME_BASE: u16 = 0x40;
pub(crate) const DW_AT_DATA_MEMBER_LOCATION: u16 = 0x38;
pub(crate) const DW_AT_COUNT: u16 = 0x37;

pub(crate) const DW_FORM_ADDR: u16 = 0x01;
pub(crate) const DW_FORM_DATA2: u16 = 0x05;
pub(crate) const DW_FORM_DATA4: u16 = 0x06;
pub(crate) const DW_FORM_DATA8: u16 = 0x07;
pub(crate) const DW_FORM_STRING: u16 = 0x08;
pub(crate) const DW_FORM_DATA1: u16 = 0x0b;
pub(crate) const DW_FORM_STRP: u16 = 0x0e;
pub(crate) const DW_FORM_REF4: u16 = 0x13;
pub(crate) const DW_FORM_SEC_OFFSET: u16 = 0x17;
pub(crate) const DW_FORM_FLAG: u16 = 0x0c;
pub(crate) const DW_FORM_BLOCK: u16 = 0x09;
pub(crate) const DW_FORM_FLAG_PRESENT: u16 = 0x19;

pub(crate) const DW_LANG_RUST: u16 = 0x001c;
pub(crate) const DW_LANG_C99: u16 = 0x000c;
pub(crate) const DW_LANG_CPP14: u16 = 0x0021;

pub(crate) const DW_ATE_BOOLEAN: u8 = 0x02;
pub(crate) const DW_ATE_FLOAT: u8 = 0x04;
pub(crate) const DW_ATE_SIGNED: u8 = 0x05;
pub(crate) const DW_ATE_UNSIGNED: u8 = 0x07;
pub(crate) const DW_ATE_UTF: u8 = 0x10;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MachHeader64 {
  pub magic: u32,
  pub cputype: u32,
  pub cpusubtype: u32,
  pub filetype: u32,
  pub ncmds: u32,
  pub sizeofcmds: u32,
  pub flags: u32,
  pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SegmentCommand64 {
  pub cmd: u32,
  pub cmdsize: u32,
  pub segname: [u8; 16],
  pub vmaddr: u64,
  pub vmsize: u64,
  pub fileoff: u64,
  pub filesize: u64,
  pub maxprot: u32,
  pub initprot: u32,
  pub nsects: u32,
  pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Section64 {
  pub sectname: [u8; 16],
  pub segname: [u8; 16],
  pub addr: u64,
  pub size: u64,
  pub offset: u32,
  pub align: u32,
  pub reloff: u32,
  pub nreloc: u32,
  pub flags: u32,
  pub reserved1: u32,
  pub reserved2: u32,
  pub reserved3: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SymtabCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub symoff: u32,
  pub nsyms: u32,
  pub stroff: u32,
  pub strsize: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DysymtabCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub ilocalsym: u32,
  pub nlocalsym: u32,
  pub iextdefsym: u32,
  pub nextdefsym: u32,
  pub iundefsym: u32,
  pub nundefsym: u32,
  pub tocoff: u32,
  pub ntoc: u32,
  pub modtaboff: u32,
  pub nmodtab: u32,
  pub extrefsymoff: u32,
  pub nextrefsyms: u32,
  pub indirectsymoff: u32,
  pub nindirectsyms: u32,
  pub extreloff: u32,
  pub nextrel: u32,
  pub locreloff: u32,
  pub nlocrel: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DylibCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub dylib: DylibStruct,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DylibStruct {
  pub name: u32, // Offset from start of DylibCommand
  pub timestamp: u32,
  pub current_version: u32,
  pub compatibility_version: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DylinkerCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub name: u32, // Offset from start of DylinkerCommand
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UuidCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub uuid: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BuildVersionCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub platform: u32,
  pub minos: u32,
  pub sdk: u32,
  pub ntools: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SourceVersionCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub version: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EntryPointCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub entryoff: u64,
  pub stacksize: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DyldInfoCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub rebase_off: u32,
  pub rebase_size: u32,
  pub bind_off: u32,
  pub bind_size: u32,
  pub weak_bind_off: u32,
  pub weak_bind_size: u32,
  pub lazy_bind_off: u32,
  pub lazy_bind_size: u32,
  pub export_off: u32,
  pub export_size: u32,
}

/// Parameters for add_dyld_info_only function
#[derive(Clone, Copy, Debug, Default)]
pub struct DyldInfo {
  pub rebase_off: u32,
  pub rebase_size: u32,
  pub bind_off: u32,
  pub bind_size: u32,
  pub weak_bind_off: u32,
  pub weak_bind_size: u32,
  pub lazy_bind_off: u32,
  pub lazy_bind_size: u32,
  pub export_off: u32,
  pub export_size: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Nlist64 {
  pub n_strx: u32,
  pub n_type: u8,
  pub n_sect: u8,
  pub n_desc: u16,
  pub n_value: u64,
}

/// Code signature load command
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LinkeditDataCommand {
  pub cmd: u32,
  pub cmdsize: u32,
  pub dataoff: u32,
  pub datasize: u32,
}

/// Code signature SuperBlob header
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SuperBlob {
  pub magic: u32, // CSMAGIC_EMBEDDED_SIGNATURE
  pub length: u32,
  pub count: u32, // Number of blobs
}

/// Blob index entry in SuperBlob
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlobIndex {
  pub type_: u32,  // Blob type (0 = CodeDirectory, etc)
  pub offset: u32, // Offset from start of SuperBlob
}

/// Code Directory structure
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectory {
  pub magic: u32, // CSMAGIC_CODEDIRECTORY
  pub length: u32,
  pub version: u32,
  pub flags: u32,
  pub hash_offset: u32, // Offset of hash slot element at index zero
  pub ident_offset: u32, // Offset of identifier string
  pub n_special_slots: u32, // Number of special hash slots
  pub n_code_slots: u32, // Number of ordinary (code) hash slots
  pub code_limit: u32,  // Limit to main image signature
  pub hash_size: u8,    // Size of each hash in bytes
  pub hash_type: u8,    // Type of hash (CS_HASHTYPE_SHA256)
  pub platform: u8,     // Platform identifier; zero if not platform binary
  pub page_shift: u8,   // log2(page size in bytes); 0 => infinite
  pub spare2: u32,      // Unused (must be zero)
  pub scatter_offset: u32, // Offset of optional scatter vector (0 if absent)
  pub team_id_offset: u32, // Offset of optional team ID string
  pub spare3: u32,
  pub code_limit_64: u64, // Limit to main image signature (64 bits)
  pub exec_seg_base: u64, // Offset of executable segment
  pub exec_seg_limit: u64, // Limit of executable segment
  pub exec_seg_flags: u64, // Flags for executable segment
}

/// Symbol visibility level
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolVisibility {
  /// Default visibility - symbol is exported
  Default,
  /// Hidden visibility - symbol is not exported
  Hidden,
  /// Private external - visible within linkage unit only
  PrivateExternal,
}

/// Symbol binding type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolBinding {
  /// Local symbol
  Local,
  /// Global symbol
  Global,
  /// Weak symbol (can be overridden)
  Weak,
  /// Lazy binding (resolved on first use)
  Lazy,
}

/// Symbol type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymbolType {
  /// No type information
  NoType,
  /// Data object
  Object,
  /// Function or code
  Function,
  /// Section symbol
  Section,
  /// Common symbol (unallocated)
  Common,
  /// Thread-local storage
  Tls,
  /// Indirect symbol
  Indirect,
  /// File symbol (source file, object file, etc.)
  File,
}

/// Symbol reference for section-relative addressing
#[derive(Clone, Copy, Debug)]
pub struct SymbolRef {
  /// Symbol index in the symbol table
  pub symbol_index: u32,
  /// Section index (1-based)
  pub section: u8,
  /// Offset within the section
  pub offset: u64,
  /// PC-relative reference
  pub is_pc_relative: bool,
}

/// Relocation entry for 64-bit architectures
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RelocationInfo {
  /// Offset in the section to the item being relocated
  pub r_address: u32,
  /// Symbol index (if r_extern = 1) or section ordinal (if r_extern = 0)
  pub r_symbolnum: u32,
  /// Indicates whether r_symbolnum is a symbol index or section ordinal
  pub r_extern: bool,
  /// Relocation type (machine-specific)
  pub r_type: u8,
  /// PC-relative relocation
  pub r_pcrel: bool,
  /// Length: 0=byte, 1=word, 2=long, 3=quad
  pub r_length: u8,
}
impl RelocationInfo {
  /// Create a new relocation entry
  pub const fn new(
    address: u32,
    symbolnum: u32,
    is_extern: bool,
    rel_type: ARM64RelocationType,
    is_pcrel: bool,
    length: u8,
  ) -> Self {
    Self {
      r_address: address,
      r_symbolnum: symbolnum,
      r_extern: is_extern,
      r_type: rel_type as u8,
      r_pcrel: is_pcrel,
      r_length: length,
    }
  }

  /// Serialize to binary format for Mach-O file
  #[inline(always)]
  pub fn to_bytes(self) -> [u8; 8] {
    let mut bytes = [0u8; 8];

    // First 4 bytes: r_address
    bytes[0..4].copy_from_slice(&self.r_address.to_le_bytes());

    // Next 4 bytes: packed fields
    // Bits 0-23: r_symbolnum (24 bits)
    // Bit 24-26: r_pcrel (1 bit)
    // Bit 27: r_extern (1 bit)
    // Bits 28-31: r_type (4 bits)
    let mut packed = self.r_symbolnum & 0x00FFFFFF;

    if self.r_pcrel {
      packed |= 1 << 24;
    }

    packed |= (self.r_length as u32 & 0x3) << 25;

    if self.r_extern {
      packed |= 1 << 27;
    }

    packed |= (self.r_type as u32 & 0xF) << 28;

    bytes[4..8].copy_from_slice(&packed.to_le_bytes());

    bytes
  }
}

/// ARM64 relocation types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ARM64RelocationType {
  /// Unsigned relocation
  Unsigned = 0,
  /// Subtractor relocation
  Subtractor = 1,
  /// Branch26 relocation (b/bl instructions)
  Branch26 = 2,
  /// Page21 relocation (adrp instruction high 21 bits)
  Page21 = 3,
  /// Pageoff12 relocation (add instruction low 12 bits)
  Pageoff12 = 4,
  /// GOT load page21 (adrp to GOT entry)
  GotLoadPage21 = 5,
  /// GOT load pageoff12 (ldr from GOT entry)
  GotLoadPageoff12 = 6,
  /// Pointer to GOT
  PointerToGot = 7,
  /// TLV page21 (adrp to TLV entry)
  TlvpLoadPage21 = 8,
  /// TLV pageoff12 (ldr from TLV entry)
  TlvpLoadPageoff12 = 9,
  /// Addend relocation
  Addend = 10,
}

/// Debug Information Entry (DIE) for DWARF
#[derive(Clone, Debug)]
pub struct DebugInfoEntry {
  /// DIE tag (DW_TAG_*)
  pub tag: u16,
  /// Attributes for this DIE
  pub attributes: Vec<DwarfAttribute>,
  /// Child DIEs
  pub children: Vec<DebugInfoEntry>,
}

/// DWARF attribute
#[derive(Clone, Debug)]
pub struct DwarfAttribute {
  /// Attribute name (DW_AT_*)
  pub name: u16,
  /// Attribute form (DW_FORM_*)
  pub form: u16,
  /// Attribute value
  pub value: DwarfValue,
}

/// DWARF attribute value
#[derive(Clone, Debug)]
pub enum DwarfValue {
  /// Address value
  Address(u64),
  /// Unsigned integer
  Data1(u8),
  Data2(u16),
  Data4(u32),
  Data8(u64),
  /// String value
  String(String),
  /// Reference to string table
  StringRef(u32),
  /// Reference to another DIE
  Reference(u32),
  /// Section offset
  SecOffset(u32),
  /// Flag (boolean)
  Flag(bool),
  /// Block of bytes
  Block(Vec<u8>),
}

/// Line number program state
#[derive(Clone, Copy, Debug)]
pub struct LineNumberProgramState {
  /// Current address
  pub address: u64,
  /// Current file index
  pub file: u32,
  /// Current line number
  pub line: u32,
  /// Current column number
  pub column: u32,
  /// Is statement
  pub is_stmt: bool,
  /// Basic block flag
  pub basic_block: bool,
  /// End sequence flag
  pub end_sequence: bool,
}
impl LineNumberProgramState {
  /// Create a new line number program state with defaults
  pub const fn new() -> Self {
    Self {
      address: 0,
      file: 1,
      line: 1,
      column: 0,
      is_stmt: true,
      basic_block: false,
      end_sequence: false,
    }
  }

  /// Generate DWARF line number program opcodes
  pub fn generate_line_program(
    &mut self,
    entries: &[DebugLineEntry],
  ) -> Vec<u8> {
    let mut program = Vec::new();

    // DWARF line number program opcodes
    const DW_LNS_COPY: u8 = 0x01;
    const DW_LNS_ADVANCE_PC: u8 = 0x02;
    const DW_LNS_ADVANCE_LINE: u8 = 0x03;
    const DW_LNS_SET_FILE: u8 = 0x04;
    const DW_LNS_SET_COLUMN: u8 = 0x05;
    const DW_LNE_END_SEQUENCE: u8 = 0x01;
    const DW_LNE_SET_ADDRESS: u8 = 0x02;

    for entry in entries {
      // Set address if changed
      if entry.address != self.address {
        // Extended opcode: set address
        program.push(0); // Extended opcode indicator
        program.push(9); // Length of extended opcode
        program.push(DW_LNE_SET_ADDRESS);
        program.extend_from_slice(&entry.address.to_le_bytes());

        self.address = entry.address;
      }

      // Set file if changed
      if entry.file_index != self.file {
        program.push(DW_LNS_SET_FILE);

        // LEB128 encode the file index
        let mut value = entry.file_index;

        while value >= 0x80 {
          program.push(((value & 0x7F) | 0x80) as u8);
          value >>= 7;
        }

        program.push(value as u8);

        self.file = entry.file_index;
      }

      // Set line if changed
      if entry.line != self.line {
        program.push(DW_LNS_ADVANCE_LINE);
        // SLEB128 encode the line difference
        let diff = (entry.line as i32) - (self.line as i32);
        let mut value = diff;

        loop {
          let byte = (value & 0x7F) as u8;

          value >>= 7;

          if (value == 0 && (byte & 0x40) == 0)
            || (value == -1 && (byte & 0x40) != 0)
          {
            program.push(byte);
            break;
          } else {
            program.push(byte | 0x80);
          }
        }

        self.line = entry.line;
      }

      // Set column if specified
      if entry.column > 0 && entry.column != self.column {
        program.push(DW_LNS_SET_COLUMN);

        // LEB128 encode the column
        let mut value = entry.column;

        while value >= 0x80 {
          program.push(((value & 0x7F) | 0x80) as u8);
          value >>= 7;
        }

        program.push(value as u8);

        self.column = entry.column;
      }

      // Handle is_stmt flag changes
      if entry.is_stmt != self.is_stmt {
        program.push(if entry.is_stmt { 0x06 } else { 0x07 }); // DW_LNS_NEGATE_STMT

        self.is_stmt = entry.is_stmt;
      }

      // Set basic_block flag if entry marks a basic block boundary
      if self.basic_block {
        program.push(0x08); // DW_LNS_SET_BASIC_BLOCK

        self.basic_block = false; // Reset after use
      }

      // Advance PC if needed (use DW_LNS_ADVANCE_PC for small increments)
      if self.address > 0 {
        let pc_advance = 1; // Example: advance by 1 instruction

        program.push(DW_LNS_ADVANCE_PC);
        program.push(pc_advance);
      }

      // Copy row to matrix
      program.push(DW_LNS_COPY);
    }

    // End sequence - mark end_sequence flag
    self.end_sequence = true;

    program.push(0); // Extended opcode
    program.push(1); // Length
    program.push(DW_LNE_END_SEQUENCE);

    program
  }
}

/// Debug line entry
#[derive(Clone, Copy, Debug)]
pub struct DebugLineEntry {
  /// Address of the instruction
  pub address: u64,
  /// Source file index
  pub file_index: u32,
  /// Line number
  pub line: u32,
  /// Column number
  pub column: u32,
  /// Is a statement
  pub is_stmt: bool,
}

/// Debug frame entry for unwinding
#[derive(Clone, Debug)]
pub struct DebugFrameEntry {
  /// Start address of function
  pub start_addr: u64,
  /// Size of function
  pub size: u64,
  /// CFA (Canonical Frame Address) instructions
  pub cfa_instructions: Vec<u8>,
}
impl DebugFrameEntry {
  /// Create a new debug frame entry
  pub fn new(start_addr: u64, size: u64) -> Self {
    Self {
      start_addr,
      size,
      cfa_instructions: Vec::new(),
    }
  }

  /// Add a DW_CFA_def_cfa instruction (define CFA as register + offset)
  #[inline(always)]
  pub fn add_def_cfa(&mut self, register: u8, offset: u64) {
    self.cfa_instructions.push(0x0c); // DW_CFA_def_cfa
    self.cfa_instructions.push(register);
    self.add_uleb128(offset);
  }

  /// Add a DW_CFA_def_cfa_offset instruction (define CFA offset)
  #[inline(always)]
  pub fn add_def_cfa_offset(&mut self, offset: u64) {
    self.cfa_instructions.push(0x0e); // DW_CFA_def_cfa_offset
    self.add_uleb128(offset);
  }

  /// Add a DW_CFA_def_cfa_register instruction (define CFA register)
  #[inline(always)]
  pub fn add_def_cfa_register(&mut self, register: u8) {
    self.cfa_instructions.push(0x0d); // DW_CFA_def_cfa_register
    self.add_uleb128(register as u64);
  }

  /// Add a DW_CFA_offset instruction (register saved at CFA + offset)
  #[inline(always)]
  pub fn add_offset(&mut self, register: u8, offset: u64) {
    if register < 64 {
      // Use compact form for registers 0-63
      self.cfa_instructions.push(0x80 | register); // DW_CFA_offset + register
      self.add_uleb128(offset);
    } else {
      // Use extended form
      self.cfa_instructions.push(0x05); // DW_CFA_offset_extended
      self.add_uleb128(register as u64);
      self.add_uleb128(offset);
    }
  }

  /// Add a DW_CFA_advance_loc instruction (advance location)
  #[inline(always)]
  pub fn add_advance_loc(&mut self, delta: u8) {
    if delta < 64 {
      // Use compact form
      self.cfa_instructions.push(0x40 | delta); // DW_CFA_advance_loc + delta
    } else {
      // Use extended forms
      self.cfa_instructions.push(0x02); // DW_CFA_advance_loc1
      self.cfa_instructions.push(delta);
    }
  }

  /// Add a DW_CFA_restore instruction (restore register to initial state)
  #[inline(always)]
  pub fn add_restore(&mut self, register: u8) {
    if register < 64 {
      // Use compact form
      self.cfa_instructions.push(0xc0 | register); // DW_CFA_restore + register
    } else {
      // Use extended form
      self.cfa_instructions.push(0x06); // DW_CFA_restore_extended
      self.add_uleb128(register as u64);
    }
  }

  /// Add a DW_CFA_nop instruction (no operation)
  #[inline(always)]
  pub fn add_nop(&mut self) {
    self.cfa_instructions.push(0x00); // DW_CFA_nop
  }

  /// Add ULEB128 encoded value
  #[inline(always)]
  fn add_uleb128(&mut self, mut value: u64) {
    loop {
      let byte = (value & 0x7f) as u8;
      value >>= 7;
      if value != 0 {
        self.cfa_instructions.push(byte | 0x80);
      } else {
        self.cfa_instructions.push(byte);
        break;
      }
    }
  }

  /// Generate frame description entry (FDE) bytes.
  #[inline(always)]
  pub fn to_fde_bytes(&self, cie_offset: u32) -> Vec<u8> {
    let mut fde = Vec::new();

    // FDE header
    let fde_length = 16 + self.cfa_instructions.len(); // Approximate length

    fde.extend_from_slice(&(fde_length as u32).to_le_bytes()); // Length
    fde.extend_from_slice(&cie_offset.to_le_bytes()); // CIE pointer
    fde.extend_from_slice(&self.start_addr.to_le_bytes()); // Initial location
    fde.extend_from_slice(&self.size.to_le_bytes()); // Address range

    // CFA instructions
    fde.extend_from_slice(&self.cfa_instructions);

    // Pad to alignment
    while fde.len() % 8 != 0 {
      fde.push(0x00); // DW_CFA_nop for padding
    }

    fde
  }
}

/// Symbol entry in the Mach-O file
#[derive(Clone, Debug)]
pub struct Symbol {
  name: String,
  n_type: u8,
  n_sect: u8,
  n_desc: u16,
  n_value: u64,
  /// Additional metadata for enhanced symbol resolution
  visibility: SymbolVisibility,
  binding: SymbolBinding,
  sym_type: SymbolType,
  /// Version string for dylib symbols
  version: Option<String>,
  /// Demangled name if applicable
  demangled_name: Option<String>,
}

/// Fat binary header structure
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FatHeader {
  /// Magic number (FAT_MAGIC or FAT_MAGIC_64)
  pub magic: u32,
  /// Number of architectures
  pub nfat_arch: u32,
}

/// Fat architecture descriptor (32-bit version)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FatArch {
  /// CPU type (e.g., CPU_TYPE_X86_64, CPU_TYPE_ARM64)
  pub cputype: u32,
  /// CPU subtype
  pub cpusubtype: u32,
  /// File offset to this architecture
  pub offset: u32,
  /// Size of this architecture
  pub size: u32,
  /// Alignment as a power of 2
  pub align: u32,
}

/// Fat architecture descriptor (64-bit version)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FatArch64 {
  /// CPU type
  pub cputype: u32,
  /// CPU subtype
  pub cpusubtype: u32,
  /// File offset to this architecture (64-bit)
  pub offset: u64,
  /// Size of this architecture (64-bit)
  pub size: u64,
  /// Alignment as a power of 2
  pub align: u32,
  /// Reserved for future use
  pub reserved: u32,
}

/// Requirements blob for code signing
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RequirementsBlob {
  /// Magic number (CSMAGIC_REQUIREMENTS)
  pub magic: u32,
  /// Total length of blob
  pub length: u32,
  /// Count of requirements
  pub count: u32,
  // Followed by BlobIndex entries and requirement data
}

/// Entitlements blob for code signing
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EntitlementsBlob {
  /// Magic number (CSMAGIC_ENTITLEMENTS)
  pub magic: u32,
  /// Total length of blob
  pub length: u32,
  // Followed by XML plist data
}

/// Main struct for building Mach-O executables
///
/// Manages the construction of a Mach-O binary including header, segments,
/// sections, and load commands.
pub struct MachO {
  header: MachHeader64,
  segments: Vec<SegmentCommand64>,
  sections: Vec<Section64>,
  /// Single buffer for all load commands (Graydon's approach)
  load_commands_buf: Vec<u8>,
  /// Offsets where each load command starts
  load_command_offsets: Vec<usize>,
  code: Vec<u8>,
  data: Vec<u8>,
  bss_size: u32,
  local_symbols: Vec<Symbol>,
  external_symbols: Vec<Symbol>,
  undefined_symbols: Vec<Symbol>,
  indirect_symbols: Vec<u32>,
  entry_point: Option<u64>,
  /// Section-relative symbol references
  symbol_refs: Vec<SymbolRef>,
  /// Symbol name to index mapping for fast lookup
  symbol_index_map: HashMap<String, u32>,
  /// Debug information entries
  debug_info: Vec<DebugInfoEntry>,
  /// Debug string table
  debug_str: Vec<u8>,
  /// Debug line information
  debug_line: Vec<DebugLineEntry>,
  /// Source files for debug info
  debug_files: Vec<String>,
  /// Debug frame entries
  debug_frame: Vec<DebugFrameEntry>,
  /// DWARF sections to be written
  dwarf_sections: Vec<(Section64, Vec<u8>)>,
  /// Text section relocations
  text_relocations: Vec<RelocationInfo>,
  /// Data section relocations
  data_relocations: Vec<RelocationInfo>,
  /// Code signing requirements (optional)
  requirements: Option<Vec<u8>>,
  /// Code signing entitlements (optional)
  entitlements: Option<String>,
}
impl MachO {
  /// Creates a new MachO builder configured for ARM64 executables
  pub fn new() -> Self {
    Self::new_with_filetype(MH_EXECUTE)
  }

  /// Creates a new MachO builder for a dynamic library
  pub fn new_dylib() -> Self {
    Self::new_with_filetype(MH_DYLIB)
  }

  /// Creates a new MachO builder for a bundle
  pub fn new_bundle() -> Self {
    Self::new_with_filetype(MH_BUNDLE)
  }

  /// Creates a new MachO builder for an object file
  pub fn new_object() -> Self {
    Self::new_with_filetype(MH_OBJECT)
  }

  /// Creates a new MachO builder for a kernel extension
  pub fn new_kext() -> Self {
    Self::new_with_filetype(MH_KEXT_BUNDLE)
  }

  /// Creates a new MachO builder with a specific file type
  pub fn new_with_filetype(filetype: u32) -> Self {
    let flags = match filetype {
      MH_EXECUTE => MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL | MH_PIE,
      MH_DYLIB => MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL,
      MH_BUNDLE => MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL,
      MH_OBJECT => MH_SUBSECTIONS_VIA_SYMBOLS,
      _ => MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL,
    };

    Self {
      header: MachHeader64 {
        magic: MH_MAGIC_64,
        cputype: CPU_TYPE_ARM64,
        cpusubtype: CPU_SUBTYPE_ARM64_ALL,
        filetype,
        ncmds: 0,
        sizeofcmds: 0,
        flags,
        reserved: 0,
      },
      segments: Vec::new(),
      sections: Vec::new(),
      load_commands_buf: Vec::with_capacity(4096), // Pre-allocate for typical usage
      load_command_offsets: Vec::with_capacity(16), // Typically 10-15 commands
      code: Vec::new(),
      data: Vec::new(),
      bss_size: 0,
      local_symbols: Vec::new(),
      external_symbols: Vec::new(),
      undefined_symbols: Vec::new(),
      indirect_symbols: Vec::new(),
      entry_point: None,
      symbol_refs: Vec::new(),
      symbol_index_map: HashMap::default(),
      debug_info: Vec::new(),
      debug_str: vec![0], // Start with null byte
      debug_line: Vec::new(),
      debug_files: Vec::new(),
      debug_frame: Vec::new(),
      dwarf_sections: Vec::new(),
      text_relocations: Vec::new(),
      data_relocations: Vec::new(),
      requirements: None,
      entitlements: None,
    }
  }

  /// Set code signing requirements
  pub fn set_requirements(&mut self, requirements: Vec<u8>) {
    self.requirements = Some(requirements);
  }

  /// Set code signing entitlements (as XML plist string)
  pub fn set_entitlements(&mut self, entitlements: String) {
    self.entitlements = Some(entitlements);
  }

  /// Generic method to add any load command (eliminates duplication)
  /// Based on Graydon's single buffer approach in macho.ml
  #[inline(always)]
  fn add_load_command<T>(&mut self, cmd: &T) {
    // Track where this command starts
    self.load_command_offsets.push(self.load_commands_buf.len());

    // Serialize directly to buffer - NO ALLOCATION!
    let size = std::mem::size_of::<T>();
    let bytes = unsafe {
      std::slice::from_raw_parts(cmd as *const T as *const u8, size)
    };
    self.load_commands_buf.extend_from_slice(bytes);
  }

  /// Add raw bytes as a load command (for commands with variable data)
  #[inline(always)]
  fn add_load_command_bytes(&mut self, bytes: &[u8]) {
    self.load_command_offsets.push(self.load_commands_buf.len());
    self.load_commands_buf.extend_from_slice(bytes);
  }

  /// Set the CPU type and subtype for the binary
  pub fn set_cpu_type(&mut self, cputype: u32, cpusubtype: u32) {
    self.header.cputype = cputype;
    self.header.cpusubtype = cpusubtype;
  }

  /// Configure for x86_64 architecture
  pub fn set_x86_64(&mut self) {
    self.set_cpu_type(CPU_TYPE_X86_64, CPU_SUBTYPE_X86_64_ALL);
  }

  /// Configure for ARM64 architecture
  pub fn set_arm64(&mut self) {
    self.set_cpu_type(CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64_ALL);
  }

  /// Configure for ARM64E architecture
  pub fn set_arm64e(&mut self) {
    self.set_cpu_type(CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64E);
  }

  /// Add a header flag
  pub fn add_flag(&mut self, flag: u32) {
    self.header.flags |= flag;
  }

  /// Remove a header flag
  pub fn remove_flag(&mut self, flag: u32) {
    self.header.flags &= !flag;
  }

  /// Set whether the binary allows stack execution
  pub fn set_allow_stack_execution(&mut self, allow: bool) {
    if allow {
      self.add_flag(MH_ALLOW_STACK_EXECUTION);
    } else {
      self.remove_flag(MH_ALLOW_STACK_EXECUTION);
    }
  }

  /// Set whether the binary has thread local variables
  pub fn set_has_tlv(&mut self, has_tlv: bool) {
    if has_tlv {
      self.add_flag(MH_HAS_TLV_DESCRIPTORS);
    } else {
      self.remove_flag(MH_HAS_TLV_DESCRIPTORS);
    }
  }

  /// Set whether the binary is app extension safe
  pub fn set_app_extension_safe(&mut self, safe: bool) {
    if safe {
      self.add_flag(MH_APP_EXTENSION_SAFE);
    } else {
      self.remove_flag(MH_APP_EXTENSION_SAFE);
    }
  }

  /// Set whether heap execution is allowed
  pub fn set_no_heap_execution(&mut self, no_heap_exec: bool) {
    if no_heap_exec {
      self.add_flag(MH_NO_HEAP_EXECUTION);
    } else {
      self.remove_flag(MH_NO_HEAP_EXECUTION);
    }
  }

  /// Adds machine code that will be placed in the __TEXT segment
  ///
  /// # Arguments
  /// * `code` - ARM64 machine code bytes
  pub fn add_code(&mut self, code: Vec<u8>) {
    self.code = code;
  }

  /// Adds data that will be placed in the __DATA segment
  ///
  /// # Arguments
  /// * `data` - Data bytes (strings, constants, etc.)
  pub fn add_data(&mut self, data: Vec<u8>) {
    self.data = data;
  }

  #[inline(always)]
  fn make_cstring(name: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];

    let bytes = name.as_bytes();
    let len = bytes.len().min(15);

    buf[..len].copy_from_slice(&bytes[..len]);

    buf
  }

  /// Adds the __PAGEZERO segment (required for 64-bit executables)
  ///
  /// This segment reserves the first 4GB of virtual memory to catch null
  /// pointer dereferences
  #[inline(always)]
  pub fn add_pagezero_segment(&mut self) {
    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32,
      segname: Self::make_cstring("__PAGEZERO"),
      vmaddr: 0,
      vmsize: VM_BASE, // Use VM_BASE constant instead of 0x100000000
      fileoff: 0,
      filesize: 0,
      maxprot: VM_PROT_NONE,
      initprot: VM_PROT_NONE,
      nsects: 0,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Create a C string literals section
  #[inline(always)]
  pub fn create_cstring_section(
    &mut self,
    segname: &str,
    sectname: &str,
    data: Vec<u8>,
  ) -> Section64 {
    Section64 {
      sectname: Self::make_cstring(sectname),
      segname: Self::make_cstring(segname),
      addr: 0, // Will be set by caller
      size: data.len() as u64,
      offset: 0, // Will be set by caller
      align: 0,  // No alignment for C strings
      reloff: 0,
      nreloc: 0,
      flags: S_CSTRING_LITERALS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    }
  }

  /// Create a symbol stubs section
  #[inline(always)]
  pub fn create_symbol_stubs_section(
    &mut self,
    segname: &str,
    sectname: &str,
    stub_size: u32,
  ) -> Section64 {
    Section64 {
      sectname: Self::make_cstring(sectname),
      segname: Self::make_cstring(segname),
      addr: 0,   // Will be set by caller
      size: 0,   // Will be set by caller
      offset: 0, // Will be set by caller
      align: 1,  // 2^1 = 2 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_SYMBOL_STUBS,
      reserved1: 0,
      reserved2: stub_size, // Size of each stub
      reserved3: 0,
    }
  }

  /// Create a lazy symbol pointers section
  #[inline(always)]
  pub fn create_lazy_symbol_pointers_section(
    &mut self,
    segname: &str,
    sectname: &str,
  ) -> Section64 {
    Section64 {
      sectname: Self::make_cstring(sectname),
      segname: Self::make_cstring(segname),
      addr: 0,   // Will be set by caller
      size: 0,   // Will be set by caller
      offset: 0, // Will be set by caller
      align: 3,  // 2^3 = 8 byte alignment for pointers
      reloff: 0,
      nreloc: 0,
      flags: S_LAZY_SYMBOL_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    }
  }

  /// Create a literal pointers section
  #[inline(always)]
  pub fn create_literal_pointers_section(
    &mut self,
    segname: &str,
    sectname: &str,
  ) -> Section64 {
    Section64 {
      sectname: Self::make_cstring(sectname),
      segname: Self::make_cstring(segname),
      addr: 0,   // Will be set by caller
      size: 0,   // Will be set by caller
      offset: 0, // Will be set by caller
      align: 3,  // 2^3 = 8 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_LITERAL_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    }
  }

  /// Adds the __TEXT segment containing executable code
  ///
  /// Creates both the segment and the __text section within it.
  #[inline(always)]
  pub fn add_text_segment(&mut self) {
    // Calculate where code starts after header and load commands
    let code_offset = CODE_OFFSET;

    let text_section = Section64 {
      sectname: Self::make_cstring("__text"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + code_offset as u64,
      size: self.code.len() as u64,
      offset: code_offset,
      align: SECTION_ALIGN_4BYTE, // 2^2 = 4 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_REGULAR | S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: (std::mem::size_of::<SegmentCommand64>()
        + std::mem::size_of::<Section64>()) as u32,
      segname: Self::make_cstring("__TEXT"),
      vmaddr: TEXT_VM_ADDR,
      vmsize: TEXT_SEGMENT_SIZE as u64,
      fileoff: TEXT_FILE_OFFSET, // TEXT segment includes header!
      filesize: TEXT_SEGMENT_SIZE as u64,
      maxprot: VM_PROT_READ | VM_PROT_EXECUTE,
      initprot: VM_PROT_READ | VM_PROT_EXECUTE,
      nsects: 1,
      flags: 0,
    };

    self.segments.push(cmd);
    self.sections.push(text_section);
  }

  /// Adds the __DATA segment containing program data
  ///
  /// Creates the segment with multiple sections: __data, __bss, __const,
  /// __nl_symbol_ptr
  #[inline(always)]
  pub fn add_data_segment(&mut self) {
    let mut sections = Vec::new();
    let mut current_offset = DATA_FILE_OFFSET as u32;
    let mut current_addr = DATA_VM_ADDR;

    // __data section
    if !self.data.is_empty() {
      sections.push(Section64 {
        sectname: Self::make_cstring("__data"),
        segname: Self::make_cstring("__DATA"),
        addr: current_addr,
        size: self.data.len() as u64,
        offset: current_offset,
        align: SECTION_ALIGN_4BYTE,
        reloff: 0,
        nreloc: 0,
        flags: S_REGULAR,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      });

      current_offset += self.data.len() as u32;
      current_addr += self.data.len() as u64;
    }

    // __bss section (zero-fill)
    if self.bss_size > 0 {
      sections.push(Section64 {
        sectname: Self::make_cstring("__bss"),
        segname: Self::make_cstring("__DATA"),
        addr: current_addr,
        size: self.bss_size as u64,
        offset: 0, // BSS has no file backing
        align: SECTION_ALIGN_4BYTE,
        reloff: 0,
        nreloc: 0,
        flags: S_ZEROFILL,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      });

      current_addr += self.bss_size as u64;
    }

    // __nl_symbol_ptr section for non-lazy symbol pointers
    if !self.indirect_symbols.is_empty() {
      let ptr_size = 8u32; // 64-bit pointers
      let ptr_count = self.indirect_symbols.len() as u32;

      sections.push(Section64 {
        sectname: Self::make_cstring("__nl_symbol_ptr"),
        segname: Self::make_cstring("__DATA"),
        addr: current_addr,
        size: (ptr_count * ptr_size) as u64,
        offset: current_offset,
        align: 3, // 2^3 = 8 byte alignment for pointers
        reloff: 0,
        nreloc: 0,
        flags: S_NON_LAZY_SYMBOL_POINTERS,
        reserved1: 0, // Index into indirect symbol table
        reserved2: 0,
        reserved3: 0,
      });
    }

    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: (std::mem::size_of::<SegmentCommand64>()
        + std::mem::size_of::<Section64>() * sections.len())
        as u32,
      segname: Self::make_cstring("__DATA"),
      vmaddr: DATA_VM_ADDR,
      vmsize: DATA_SEGMENT_SIZE as u64,
      fileoff: DATA_FILE_OFFSET,
      filesize: DATA_SEGMENT_SIZE as u64,
      maxprot: VM_PROT_READ | VM_PROT_WRITE,
      initprot: VM_PROT_READ | VM_PROT_WRITE,
      nsects: sections.len() as u32,
      flags: 0,
    };

    self.segments.push(cmd);
    self.sections.extend(sections);
  }

  /// Adds the __DATA_CONST segment for read-only data after loading
  #[inline(always)]
  pub fn add_data_const_segment(&mut self) {
    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32,
      segname: Self::make_cstring("__DATA_CONST"),
      vmaddr: DATA_VM_ADDR + 0x4000, // After __DATA
      vmsize: PAGE_SIZE as u64,
      fileoff: DATA_FILE_OFFSET + 0x4000,
      filesize: 0, // Will be updated with actual size
      maxprot: VM_PROT_READ | VM_PROT_WRITE,
      initprot: VM_PROT_READ,
      nsects: 0,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Adds the __OBJC segment for Objective-C runtime data
  #[inline(always)]
  pub fn add_objc_segment(&mut self) {
    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32,
      segname: Self::make_cstring("__OBJC"),
      vmaddr: DATA_VM_ADDR + 0x8000,
      vmsize: PAGE_SIZE as u64,
      fileoff: DATA_FILE_OFFSET + 0x8000,
      filesize: 0,
      maxprot: VM_PROT_READ | VM_PROT_WRITE,
      initprot: VM_PROT_READ | VM_PROT_WRITE,
      nsects: 0,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Adds the __IMPORT segment for import tables
  #[inline(always)]
  pub fn add_import_segment(&mut self) {
    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32,
      segname: Self::make_cstring("__IMPORT"),
      vmaddr: DATA_VM_ADDR + 0xC000,
      vmsize: PAGE_SIZE as u64,
      fileoff: DATA_FILE_OFFSET + 0xC000,
      filesize: 0,
      maxprot: VM_PROT_READ | VM_PROT_WRITE | VM_PROT_EXECUTE,
      initprot: VM_PROT_READ | VM_PROT_WRITE,
      nsects: 0,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Adds the __LINKEDIT segment for dynamic linker information
  ///
  /// # Arguments
  /// * `offset` - File offset for the segment
  /// * `size` - Size of the segment
  #[inline(always)]
  pub fn add_linkedit_segment(&mut self, offset: u32, size: u32) {
    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32,
      segname: Self::make_cstring("__LINKEDIT"),
      vmaddr: LINKEDIT_VM_ADDR,
      vmsize: size as u64,
      fileoff: offset as u64,
      filesize: size as u64,
      maxprot: VM_PROT_READ,
      initprot: VM_PROT_READ,
      nsects: 0,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Adds LC_DYLD_INFO_ONLY load command (required for dynamic linking)
  #[inline(always)]
  pub fn add_dyld_info(&mut self) {
    let cmd = DyldInfoCommand {
      cmd: LC_DYLD_INFO_ONLY,
      cmdsize: std::mem::size_of::<DyldInfoCommand>() as u32,
      rebase_off: 0,
      rebase_size: 0,
      bind_off: 0,
      bind_size: 0,
      weak_bind_off: 0,
      weak_bind_size: 0,
      lazy_bind_off: 0,
      lazy_bind_size: 0,
      export_off: 0,
      export_size: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Adds symbol table load command
  ///
  /// # Arguments
  /// * `symoff` - File offset to symbol table
  /// * `nsyms` - Number of symbols
  /// * `stroff` - File offset to string table
  /// * `strsize` - Size of string table
  #[inline(always)]
  pub fn add_symtab(
    &mut self,
    symoff: u32,
    nsyms: u32,
    stroff: u32,
    strsize: u32,
  ) {
    let cmd = SymtabCommand {
      cmd: LC_SYMTAB,
      cmdsize: std::mem::size_of::<SymtabCommand>() as u32,
      symoff,
      nsyms,
      stroff,
      strsize,
    };

    self.add_load_command(&cmd);
  }

  /// Adds dynamic symbol table load command
  #[inline(always)]
  pub fn add_dysymtab(
    &mut self,
    nlocalsym: u32,
    nextdefsym: u32,
    nundefsym: u32,
    indirectsymoff: u32,
    nindirectsyms: u32,
  ) {
    let ilocalsym = 0;
    let iextdefsym = nlocalsym;
    let iundefsym = nlocalsym + nextdefsym;

    let cmd = DysymtabCommand {
      cmd: LC_DYSYMTAB,
      cmdsize: std::mem::size_of::<DysymtabCommand>() as u32,
      ilocalsym,
      nlocalsym,
      iextdefsym,
      nextdefsym,
      iundefsym,
      nundefsym,
      tocoff: 0,
      ntoc: 0,
      modtaboff: 0,
      nmodtab: 0,
      extrefsymoff: 0,
      nextrefsyms: 0,
      indirectsymoff,
      nindirectsyms,
      extreloff: 0,
      nextrel: 0,
      locreloff: 0,
      nlocrel: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Adds the dynamic linker load command (typically /usr/lib/dyld)
  pub fn add_dylinker(&mut self) {
    let path = "/usr/lib/dyld\0";
    let path_len = path.len();

    let cmdsize = (std::mem::size_of::<DylinkerCommand>() + path_len + 7)
      & ALIGNMENT_8BYTE_MASK;

    let cmd = DylinkerCommand {
      cmd: LC_LOAD_DYLINKER,
      cmdsize: cmdsize as u32,
      name: std::mem::size_of::<DylinkerCommand>() as u32,
    };

    let mut cmd_data = unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylinkerCommand>(),
      )
      .to_vec()
    };

    cmd_data.extend_from_slice(path.as_bytes());
    cmd_data.resize(cmdsize, 0);
    self.add_load_command_bytes(&cmd_data);
  }

  /// Adds a dynamic library dependency
  ///
  /// # Arguments
  /// * `path` - Path to the dynamic library (e.g.,
  ///   "/usr/lib/libSystem.B.dylib")
  pub fn add_dylib(&mut self, path: &str) {
    let path_with_null = format!("{path}\0");
    let path_len = path_with_null.len();

    let cmdsize = (std::mem::size_of::<DylibCommand>() + path_len + 7)
      & ALIGNMENT_8BYTE_MASK;

    let cmd = DylibCommand {
      cmd: LC_LOAD_DYLIB,
      cmdsize: cmdsize as u32,
      dylib: DylibStruct {
        name: std::mem::size_of::<DylibCommand>() as u32,
        timestamp: 0,
        current_version: 0,
        compatibility_version: 0,
      },
    };

    let mut cmd_data = unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
      .to_vec()
    };

    cmd_data.extend_from_slice(path_with_null.as_bytes());
    cmd_data.resize(cmdsize, 0);
    self.add_load_command_bytes(&cmd_data);
  }

  /// Adds a UUID load command (unique identifier for the binary)
  pub fn add_uuid(&mut self) {
    // Generate a pseudo-random UUID based on timestamp and content
    // In production, you'd use a proper UUID library
    let mut uuid = [0u8; 16];

    // Use a simple hash of the code content as UUID
    // This ensures the same code produces the same UUID (reproducible builds)
    let mut hash = 0u64;

    for (i, byte) in self.code.iter().enumerate() {
      hash = hash.wrapping_mul(31).wrapping_add(*byte as u64);

      if i % 8 == 7 {
        let idx = (i / 8) * 8;

        if idx < 16 {
          let end = (idx + 8).min(16);

          uuid[idx..end].copy_from_slice(&hash.to_le_bytes()[..(end - idx)]);
        }
      }
    }

    // Set version (4) and variant bits for UUID v4 format
    uuid[6] = (uuid[6] & 0x0f) | 0x40; // Version 4
    uuid[8] = (uuid[8] & 0x3f) | 0x80; // Variant 10

    let cmd = UuidCommand {
      cmd: LC_UUID,
      cmdsize: std::mem::size_of::<UuidCommand>() as u32,
      uuid,
    };

    self.add_load_command(&cmd);
  }

  /// Adds build version information (specifies minimum OS version)
  #[inline(always)]
  pub fn add_build_version(&mut self) {
    let cmd = BuildVersionCommand {
      cmd: LC_BUILD_VERSION,
      cmdsize: std::mem::size_of::<BuildVersionCommand>() as u32,
      platform: PLATFORM_MACOS,
      minos: MACOS_VERSION_14_0,
      sdk: MACOS_VERSION_14_0,
      ntools: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Adds source version information
  #[inline(always)]
  pub fn add_source_version(&mut self) {
    let cmd = SourceVersionCommand {
      cmd: LC_SOURCE_VERSION,
      cmdsize: std::mem::size_of::<SourceVersionCommand>() as u32,
      version: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Sets the entry point for the executable
  pub fn set_entry_point(&mut self, offset: u64) {
    self.entry_point = Some(offset);
  }

  /// Adds the main entry point for the executable
  ///
  /// # Arguments
  /// * `entry_offset` - File offset of the entry point (usually start of __text
  ///   section)
  #[inline(always)]
  pub fn add_main(&mut self, entry_offset: u64) {
    let cmd = EntryPointCommand {
      cmd: LC_MAIN,
      cmdsize: std::mem::size_of::<EntryPointCommand>() as u32,
      entryoff: entry_offset,
      stacksize: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Adds a local symbol (private to this object file)
  #[inline(always)]
  pub fn add_local_symbol(&mut self, name: &str, section: u8, value: u64) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_SECT,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds an external symbol (exported from this object file)
  #[inline(always)]
  pub fn add_external_symbol(&mut self, name: &str, section: u8, value: u64) {
    self.external_symbols.push(Symbol {
      name: name.into(),
      n_type: N_SECT | N_EXT,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds an undefined symbol (imported from dynamic libraries)
  /// Add thread local variable section (__DATA,__thread_vars)
  #[inline(always)]
  pub fn add_thread_vars_section(&mut self, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__thread_vars"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3, // 8-byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_THREAD_LOCAL_VARIABLES,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add thread local data section (__DATA,__thread_data)
  #[inline(always)]
  pub fn add_thread_data_section(&mut self, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__thread_data"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_THREAD_LOCAL_REGULAR,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add ID for dynamic library (LC_ID_DYLIB) - identifies THIS dylib
  pub fn add_dylib_id(
    &mut self,
    name: &str,
    current_version: u32,
    compatibility_version: u32,
  ) {
    #[repr(C)]
    struct DylibCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
      timestamp: u32,
      current_version: u32,
      compatibility_version: u32,
    }

    let dylib_name = format!("{name}\0");
    let padded_len = (dylib_name.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<DylibCommand>() + padded_len;

    let cmd = DylibCommand {
      cmd: LC_ID_DYLIB,
      cmdsize: cmdsize as u32,
      name_offset: std::mem::size_of::<DylibCommand>() as u32,
      timestamp: 2,
      current_version,
      compatibility_version,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
    });

    cmd_data.extend_from_slice(dylib_name.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add 4-byte literals section
  #[inline(always)]
  pub fn add_4byte_literals_section(&mut self, literals: Vec<u32>) {
    let data = literals
      .iter()
      .flat_map(|lit| lit.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__literal4"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 2, // 2^2 = 4 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_4BYTE_LITERALS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.code.extend_from_slice(&data);
  }

  /// Add 8-byte literals section
  #[inline(always)]
  pub fn add_8byte_literals_section(&mut self, literals: Vec<u64>) {
    let data = literals
      .iter()
      .flat_map(|lit| lit.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__literal8"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 3, // 2^3 = 8 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_8BYTE_LITERALS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.code.extend_from_slice(&data);
  }

  /// Add 16-byte literals section
  #[inline(always)]
  pub fn add_16byte_literals_section(&mut self, literals: Vec<u128>) {
    let data = literals
      .iter()
      .flat_map(|lit| lit.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__literal16"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 4, // 2^4 = 16 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_16BYTE_LITERALS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.code.extend_from_slice(&data);
  }

  /// Add module initializer functions section
  #[inline(always)]
  pub fn add_mod_init_funcs_section(&mut self, func_ptrs: Vec<u64>) {
    let data = func_ptrs
      .iter()
      .flat_map(|ptr| ptr.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__mod_init_func"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3, // 2^3 = 8 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_MOD_INIT_FUNC_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add module terminator functions section
  #[inline(always)]
  pub fn add_mod_term_funcs_section(&mut self, func_ptrs: Vec<u64>) {
    let data = func_ptrs
      .iter()
      .flat_map(|ptr| ptr.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__mod_term_func"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3, // 2^3 = 8 byte alignment
      reloff: 0,
      nreloc: 0,
      flags: S_MOD_TERM_FUNC_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add thread local zerofill section.
  #[inline(always)]
  pub fn add_thread_local_zerofill_section(&mut self, size: u64) {
    let section = Section64 {
      sectname: Self::make_cstring("__thread_bss"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size,
      offset: 0, // Zerofill has no file offset
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_THREAD_LOCAL_ZEROFILL,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
  }

  /// Add thread local variable pointers section.
  #[inline(always)]
  pub fn add_thread_local_var_ptrs_section(&mut self, ptrs: Vec<u64>) {
    let data = ptrs
      .iter()
      .flat_map(|ptr| ptr.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__thread_ptrs"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_THREAD_LOCAL_VARIABLE_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add two-level namespace hint
  #[inline(always)]
  pub fn add_twolevel_hint(&mut self, from_dylib: u16, symbol_index: u32) {
    // Two-level hints improve dynamic linking performance
    // Store hint for later processing in indirect symbol table
    self
      .indirect_symbols
      .push((from_dylib as u32) << 16 | symbol_index);
  }

  #[inline(always)]
  pub fn add_undefined_symbol(&mut self, name: &str, dylib_ordinal: u16) {
    self.undefined_symbols.push(Symbol {
      name: name.into(),
      n_type: N_UNDF | N_EXT,
      n_sect: 0,
      n_desc: (dylib_ordinal << 8) | REFERENCE_FLAG_UNDEFINED_NON_LAZY,
      n_value: 0,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds a weak external symbol.
  #[inline(always)]
  pub fn add_weak_symbol(&mut self, name: &str, section: u8, value: u64) {
    self.external_symbols.push(Symbol {
      name: name.into(),
      n_type: N_SECT | N_EXT,
      n_sect: section,
      n_desc: N_WEAK_DEF,
      n_value: value,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Weak,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds a private external symbol (visible only within the linkage unit).
  #[inline(always)]
  pub fn add_private_extern_symbol(
    &mut self,
    name: &str,
    section: u8,
    value: u64,
  ) {
    self.external_symbols.push(Symbol {
      name: name.into(),
      n_type: N_SECT | N_PEXT,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::PrivateExternal,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds a common symbol (uninitialized data).
  #[inline(always)]
  pub fn add_common_symbol(&mut self, name: &str, size: u64, align: u8) {
    // Common symbols use n_value for size and n_desc bits 8-15 for alignment
    let align_desc = ((align as u16) & 0x0f) << 8;

    self.external_symbols.push(Symbol {
      name: name.into(),
      n_type: N_UNDF | N_EXT,
      n_sect: 0,
      n_desc: align_desc,
      n_value: size, // Size of common symbol
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::Common,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds an undefined weak symbol (can be missing at runtime).
  #[inline(always)]
  pub fn add_undefined_weak_symbol(&mut self, name: &str, dylib_ordinal: u16) {
    // Weak references use N_WEAK_REF, not N_WEAK_DEF
    // Also use proper reference flag for weak undefined
    self.undefined_symbols.push(Symbol {
      name: name.into(),
      n_type: N_UNDF | N_EXT,
      n_sect: 0,
      n_desc: (dylib_ordinal << 8)
        | N_WEAK_REF
        | REFERENCE_FLAG_UNDEFINED_NON_LAZY,
      n_value: 0,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Weak,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add a symbol that should not be dead-stripped
  pub fn add_no_dead_strip_symbol(
    &mut self,
    name: &str,
    section: u8,
    value: u64,
  ) {
    let mut symbol = Self::create_symbol(
      name,
      section,
      value,
      SymbolVisibility::Default,
      SymbolBinding::Global,
      SymbolType::NoType,
    );

    // Mark with N_NO_DEAD_STRIP flag
    symbol.n_desc |= N_NO_DEAD_STRIP;

    if section == 0 {
      self.undefined_symbols.push(symbol);
    } else {
      self.external_symbols.push(symbol);
    }
  }

  /// Add a resolver function symbol.
  #[inline(always)]
  pub fn add_resolver_symbol(&mut self, name: &str, section: u8, value: u64) {
    let mut symbol = Self::create_symbol(
      name,
      section,
      value,
      SymbolVisibility::Default,
      SymbolBinding::Global,
      SymbolType::Function,
    );

    // Mark as resolver function
    symbol.n_desc |= N_SYMBOL_RESOLVER;

    self.external_symbols.push(symbol);
  }

  /// Add a prebound undefined symbol (from a specific dylib).
  #[inline(always)]
  pub fn add_prebound_symbol(
    &mut self,
    name: &str,
    dylib_ordinal: u16,
    value: u64,
  ) {
    self.undefined_symbols.push(Symbol {
      name: name.into(),
      n_type: N_PBUD | N_EXT, // Use N_PBUD for prebound
      n_sect: 0,
      n_desc: (dylib_ordinal << 8) | REFERENCE_FLAG_UNDEFINED_NON_LAZY,
      n_value: value, // Prebound address
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: Self::try_demangle(name),
    });
  }

  /// Adds a lazy-bound undefined symbol.
  #[inline(always)]
  pub fn add_lazy_symbol(&mut self, name: &str, dylib_ordinal: u16) {
    self.undefined_symbols.push(Symbol {
      name: name.into(),
      n_type: N_UNDF | N_EXT,
      n_sect: 0,
      n_desc: (dylib_ordinal << 8) | REFERENCE_FLAG_UNDEFINED_LAZY,
      n_value: 0,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Lazy,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Adds the __bss section size
  pub fn set_bss_size(&mut self, size: u32) {
    self.bss_size = size;
  }

  /// Adds an indirect symbol table entry
  pub fn add_indirect_symbol(&mut self, symbol_index: u32) {
    self.indirect_symbols.push(symbol_index);
  }

  /// Check if a symbol is a debug/stab symbol
  pub fn is_stab_symbol(n_type: u8) -> bool {
    (n_type & N_STAB) != 0
  }

  /// Extract the base type from n_type (strips flags)
  pub fn get_symbol_type(n_type: u8) -> u8 {
    n_type & N_TYPE
  }

  /// Create a symbol with full control over attributes
  pub fn create_symbol(
    name: &str,
    section: u8,
    value: u64,
    visibility: SymbolVisibility,
    binding: SymbolBinding,
    sym_type: SymbolType,
  ) -> Symbol {
    // Set type based on section
    let mut n_type = if section == 0 {
      N_UNDF
    } else if section == 0xff {
      N_ABS
    } else {
      N_SECT
    };

    let mut n_desc = 0u16;

    // Apply visibility
    match visibility {
      SymbolVisibility::Default => n_type |= N_EXT,
      SymbolVisibility::PrivateExternal => n_type |= N_PEXT,
      SymbolVisibility::Hidden => {} // Local symbol, no flags
    }

    // Apply binding and reference type
    match binding {
      SymbolBinding::Weak => {
        if section == 0 {
          // Undefined weak reference
          n_desc |= N_WEAK_REF;
        } else {
          // Weak definition
          n_desc |= N_WEAK_DEF;
        }
      }
      SymbolBinding::Lazy => {
        // Lazy binding for undefined symbols
        if section == 0 {
          n_desc = (n_desc & 0xFFF0) | REFERENCE_FLAG_UNDEFINED_LAZY;
        }
      }
      SymbolBinding::Local => {
        // Private/local symbols
        if section == 0 {
          n_desc =
            (n_desc & 0xFFF0) | REFERENCE_FLAG_PRIVATE_UNDEFINED_NON_LAZY;
        } else {
          n_desc = (n_desc & 0xFFF0) | REFERENCE_FLAG_PRIVATE_DEFINED;
        }
      }
      SymbolBinding::Global => {
        // Global symbols
        if section == 0 {
          n_desc = (n_desc & 0xFFF0) | REFERENCE_FLAG_UNDEFINED_NON_LAZY;
        } else {
          n_desc = (n_desc & 0xFFF0) | REFERENCE_FLAG_DEFINED;
        }
      }
    }

    // Handle special types
    match sym_type {
      SymbolType::Common => {
        n_type = N_UNDF | N_EXT;
        // value contains size for common symbols
      }
      SymbolType::Indirect => {
        n_type = N_INDR | N_EXT;
      }
      _ => {}
    }

    Symbol {
      name: name.into(),
      n_type,
      n_sect: section,
      n_desc,
      n_value: value,
      visibility,
      binding,
      sym_type,
      version: None,
      demangled_name: Self::try_demangle(name),
    }
  }

  /// Attempts to demangle a symbol name (simplified version)
  /// In production, you'd use a proper demangling library
  #[inline(always)]
  fn try_demangle(name: &str) -> Option<String> {
    // Simple Rust demangling heuristic
    if name.starts_with("_ZN") || name.starts_with("__ZN") {
      // This is a mangled Rust symbol
      // In production, use rustc_demangle crate
      Some(format!("<demangled: {name}>"))
    }
    // C++ demangling
    else if name.starts_with("_Z") || name.starts_with("__Z") {
      // This is a mangled C++ symbol
      // In production, use cpp_demangle crate
      Some(format!("<demangled: {name}>"))
    }
    // Swift demangling
    else if name.starts_with("_$s") || name.starts_with("_$S") {
      // This is a mangled Swift symbol
      Some(format!("<demangled: {name}>"))
    } else {
      None
    }
  }

  /// Sets version information for a symbol
  pub fn set_symbol_version(symbol: &mut Symbol, version: &str) {
    symbol.version = Some(version.into());
  }

  /// Add a re-exported symbol from another dylib.
  #[inline(always)]
  pub fn add_reexport_symbol(
    &mut self,
    name: &str,
    source_dylib: &str,
    dylib_ordinal: u16,
  ) {
    // Re-exports use special undefined symbols with library ordinal
    let mut symbol = Symbol {
      name: name.into(),
      n_type: N_UNDF | N_EXT,
      n_sect: 0,
      n_desc: (dylib_ordinal << 8) | REFERENCE_FLAG_UNDEFINED_NON_LAZY,
      n_value: 0,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::NoType,
      version: Some(format!("from {source_dylib}")),
      demangled_name: Self::try_demangle(name),
    };

    // Mark as re-export in description
    symbol.n_desc |= 0x0008; // EXPORT_SYMBOL_FLAGS_REEXPORT

    self.undefined_symbols.push(symbol);
  }

  /// Add a section-relative symbol reference
  pub fn add_symbol_ref(
    &mut self,
    symbol_name: &str,
    section: u8,
    offset: u64,
    is_pc_relative: bool,
  ) -> u32 {
    // Find or create symbol index
    let symbol_index =
      if let Some(&idx) = self.symbol_index_map.get(symbol_name) {
        idx
      } else {
        // Calculate the next symbol index
        let idx = (self.local_symbols.len()
          + self.external_symbols.len()
          + self.undefined_symbols.len()) as u32;
        self.symbol_index_map.insert(symbol_name.into(), idx);
        idx
      };

    self.symbol_refs.push(SymbolRef {
      symbol_index,
      section,
      offset,
      is_pc_relative,
    });

    symbol_index
  }

  /// Get symbol by name (for fast lookup)
  #[inline(always)]
  pub fn find_symbol(&self, name: &str) -> Option<u32> {
    self.symbol_index_map.get(name).copied()
  }

  /// Generate indirect symbol table from symbol references.
  #[inline(always)]
  fn generate_indirect_symbol_table(&self) -> Vec<u32> {
    let mut indirect_symbols = Vec::new();

    // Process all symbol references
    for sym_ref in &self.symbol_refs {
      // Only add PC-relative references to indirect table
      if sym_ref.is_pc_relative {
        indirect_symbols.push(sym_ref.symbol_index);
      }
    }

    // Also check section-specific references
    for sym_ref in &self.symbol_refs {
      if sym_ref.section > 0 && sym_ref.offset > 0 {
        // This is a section-relative reference
        indirect_symbols.push(sym_ref.symbol_index);
      }
    }

    indirect_symbols
  }

  /// Build symbol index map for fast lookup
  /// Call this after adding all symbols but before finish().
  #[inline(always)]
  pub fn build_symbol_index(&mut self) {
    self.symbol_index_map.clear();

    let mut index = 0u32;

    // Index local symbols
    for symbol in &self.local_symbols {
      self.symbol_index_map.insert(symbol.name.clone(), index);

      index += 1;
    }

    // Index external symbols
    for symbol in &self.external_symbols {
      self.symbol_index_map.insert(symbol.name.clone(), index);

      index += 1;
    }

    // Index undefined symbols
    for symbol in &self.undefined_symbols {
      self.symbol_index_map.insert(symbol.name.clone(), index);

      index += 1;
    }
  }

  /// Add a function symbol with proper attributes.
  #[inline(always)]
  pub fn add_function_symbol(
    &mut self,
    name: &str,
    section: u8,
    addr: u64,
    is_thumb: bool,
  ) {
    let mut symbol = Self::create_symbol(
      name,
      section,
      addr,
      SymbolVisibility::Default,
      SymbolBinding::Global,
      SymbolType::Function,
    );

    // Mark ARM Thumb functions
    if is_thumb {
      symbol.n_desc |= N_ARM_THUMB_DEF;
      // Thumb functions have bit 0 set in address
      symbol.n_value |= 1;
    }

    self.external_symbols.push(symbol);
  }

  /// Add a data object symbol.
  #[inline(always)]
  pub fn add_data_symbol(
    &mut self,
    name: &str,
    section: u8,
    addr: u64,
    size: u64,
    visibility: SymbolVisibility,
  ) {
    // For common symbols (uninitialized data), size goes in n_value
    let (final_section, final_value) = if section == 0 && size > 0 {
      // This is a common symbol - size goes in n_value
      (0, size)
    } else {
      // Regular symbol - address goes in n_value
      (section, addr)
    };

    let mut symbol = Self::create_symbol(
      name,
      final_section,
      final_value,
      visibility,
      SymbolBinding::Global,
      if section == 0 && size > 0 {
        SymbolType::Common
      } else {
        SymbolType::Object
      },
    );

    // For common symbols, encode alignment in n_desc bits 8-15
    if section == 0 && size > 0 {
      // Calculate alignment (default to 3 for 8-byte alignment)
      let align = 3u16; // 2^3 = 8 bytes

      symbol.n_desc = (symbol.n_desc & 0xF0FF) | ((align & 0x0F) << 8);
    }

    match visibility {
      SymbolVisibility::Hidden | SymbolVisibility::PrivateExternal => {
        self.local_symbols.push(symbol);
      }
      SymbolVisibility::Default => {
        if section == 0 {
          self.undefined_symbols.push(symbol);
        } else {
          self.external_symbols.push(symbol);
        }
      }
    }
  }

  /// Add a global debug symbol.
  #[inline(always)]
  pub fn add_global_debug_symbol(
    &mut self,
    name: &str,
    section: u8,
    value: u64,
  ) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_GSYM | N_EXT,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::Function,
      version: None,
      demangled_name: None,
    });
  }

  /// Add a local debug symbol.
  #[inline(always)]
  pub fn add_local_debug_symbol(
    &mut self,
    name: &str,
    section: u8,
    value: u64,
  ) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_LSYM,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Object,
      version: None,
      demangled_name: None,
    });
  }

  /// Add a static debug symbol.
  #[inline(always)]
  pub fn add_static_debug_symbol(
    &mut self,
    name: &str,
    section: u8,
    value: u64,
  ) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_STSYM,
      n_sect: section,
      n_desc: 0,
      n_value: value,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Object,
      version: None,
      demangled_name: None,
    });
  }

  /// Add source file debug info.
  #[inline(always)]
  pub fn add_source_file_info(&mut self, filename: &str, compile_dir: &str) {
    // Add compile directory
    self.local_symbols.push(Symbol {
      name: compile_dir.into(),
      n_type: N_SO,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });

    // Add source file
    self.local_symbols.push(Symbol {
      name: filename.into(),
      n_type: N_SO,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });
  }

  /// Add object file info.
  #[inline(always)]
  pub fn add_object_file_info(&mut self, obj_file: &str, timestamp: u64) {
    self.local_symbols.push(Symbol {
      name: obj_file.into(),
      n_type: N_OSO,
      n_sect: 0,
      n_desc: 1, // SDK version
      n_value: timestamp,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });
  }

  /// Add function begin/end bracket symbols
  pub fn add_function_brackets(
    &mut self,
    name: &str,
    section: u8,
    start: u64,
    size: u64,
  ) {
    // Begin function bracket
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_BNSYM,
      n_sect: section,
      n_desc: 0,
      n_value: start,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });

    // Function symbol
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_FUN,
      n_sect: section,
      n_desc: 0,
      n_value: start,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Function,
      version: None,
      demangled_name: None,
    });

    // End function (empty name, size in n_value)
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_FUN,
      n_sect: 0,
      n_desc: 0,
      n_value: size,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });

    // End function bracket
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_ENSYM,
      n_sect: section,
      n_desc: 0,
      n_value: start + size,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add source line debug info.
  #[inline(always)]
  pub fn add_source_line(&mut self, line: u16, address: u64) {
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_SLINE,
      n_sect: 1, // __TEXT,__text
      n_desc: line,
      n_value: address,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add parameter symbol.
  #[inline(always)]
  pub fn add_parameter_symbol(&mut self, name: &str, stack_offset: i32) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_PSYM,
      n_sect: 0,
      n_desc: 0,
      n_value: stack_offset as u64,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add compiler optimization info.
  #[inline(always)]
  pub fn add_compiler_info(&mut self, version: &str, opt_level: u8) {
    // Compiler version
    self.local_symbols.push(Symbol {
      name: version.into(),
      n_type: N_VERSION,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });

    // Optimization level
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_OLEVEL,
      n_sect: 0,
      n_desc: opt_level as u16,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add common block symbols.
  #[inline(always)]
  pub fn add_common_block(&mut self, name: &str, size: u64) {
    // Begin common
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_BCOMM,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });

    // End common
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_ECOMM,
      n_sect: 0,
      n_desc: 0,
      n_value: size,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add local common block (with ECOML).
  #[inline(always)]
  pub fn add_local_common_block(&mut self, name: &str, size: u64) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_LCSYM,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Common,
      version: None,
      demangled_name: None,
    });

    // End with local name
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_ECOML,
      n_sect: 0,
      n_desc: 0,
      n_value: size,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add function name symbol (FNAME).
  #[inline(always)]
  pub fn add_function_name(&mut self, name: &str) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_FNAME,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Function,
      version: None,
      demangled_name: None,
    });
  }

  /// Add register symbol (RSYM).
  #[inline(always)]
  pub fn add_register_symbol(&mut self, name: &str, register: u16) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_RSYM,
      n_sect: 0,
      n_desc: register,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add structure symbol (SSYM).
  #[inline(always)]
  pub fn add_structure_symbol(&mut self, name: &str, size: u64) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_SSYM,
      n_sect: 0,
      n_desc: 0,
      n_value: size,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Object,
      version: None,
      demangled_name: None,
    });
  }

  /// Add debugger options (OPT).
  #[inline(always)]
  pub fn add_debugger_options(&mut self, options: &str) {
    self.local_symbols.push(Symbol {
      name: options.into(),
      n_type: N_OPT,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add included source file (SOL).
  #[inline(always)]
  pub fn add_included_source(&mut self, filename: &str) {
    self.local_symbols.push(Symbol {
      name: filename.into(),
      n_type: N_SOL,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });
  }

  /// Add compiler parameters (PARAMS).
  #[inline(always)]
  pub fn add_compiler_params(&mut self, params: &str) {
    self.local_symbols.push(Symbol {
      name: params.into(),
      n_type: N_PARAMS,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add include file with begin/end markers.
  #[inline(always)]
  pub fn add_include_file_markers(&mut self, filename: &str) {
    // Begin include
    self.local_symbols.push(Symbol {
      name: filename.into(),
      n_type: N_BINCL,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });
  }

  /// End include file marker.
  #[inline(always)]
  pub fn end_include_file_marker(&mut self) {
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_EINCL,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add alternate entry point (ENTRY).
  #[inline(always)]
  pub fn add_alternate_entry(&mut self, name: &str, address: u64) {
    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: N_ENTRY,
      n_sect: 1, // __TEXT
      n_desc: 0,
      n_value: address,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::Function,
      version: None,
      demangled_name: None,
    });
  }

  /// Add excluded include file (EXCL).
  #[inline(always)]
  pub fn add_excluded_include(&mut self, filename: &str) {
    self.local_symbols.push(Symbol {
      name: filename.into(),
      n_type: N_EXCL,
      n_sect: 0,
      n_desc: 0,
      n_value: 0,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::File,
      version: None,
      demangled_name: None,
    });
  }

  /// Add length of preceding entry (LENG).
  #[inline(always)]
  pub fn add_symbol_length(&mut self, length: u64) {
    self.local_symbols.push(Symbol {
      name: String::new(),
      n_type: N_LENG,
      n_sect: 0,
      n_desc: 0,
      n_value: length,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    });
  }

  /// Add private undefined lazy symbol.
  #[inline(always)]
  pub fn add_private_undefined_lazy_symbol(&mut self, name: &str) {
    let mut symbol = Symbol {
      name: name.into(),
      n_type: N_UNDF | N_PEXT,
      n_sect: 0,
      n_desc: REFERENCE_FLAG_PRIVATE_UNDEFINED_LAZY,
      n_value: 0,
      visibility: SymbolVisibility::PrivateExternal,
      binding: SymbolBinding::Weak,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None,
    };
    symbol.n_desc |= N_REF_TO_WEAK;
    self.undefined_symbols.push(symbol);
  }

  /// Add an indirect symbol (points to another symbol).
  #[inline(always)]
  pub fn add_indirect_symbol_entry(&mut self, name: &str, target_name: &str) {
    // Indirect symbols use N_INDR type
    let mut symbol = Symbol {
      name: name.into(),
      n_type: N_INDR | N_EXT,
      n_sect: 0,
      n_desc: 0,
      n_value: 0, // Will be set to string table index of target
      visibility: SymbolVisibility::Default,
      binding: SymbolBinding::Global,
      sym_type: SymbolType::Indirect,
      version: None,
      demangled_name: Self::try_demangle(name),
    };

    // Store target name in version field for now
    // In finish(), we'll resolve this to the actual string table offset
    symbol.version = Some(target_name.into());

    self.external_symbols.push(symbol);
  }

  /// Add a debug symbol (STAB entry).
  #[inline(always)]
  pub fn add_debug_symbol(
    &mut self,
    stab_type: u8,
    name: &str,
    section: u8,
    desc: u16,
    value: u64,
  ) {
    // Debug symbols have N_STAB bit set
    debug_assert!((stab_type & N_STAB) != 0, "Invalid STAB type");

    self.local_symbols.push(Symbol {
      name: name.into(),
      n_type: stab_type,
      n_sect: section,
      n_desc: desc,
      n_value: value,
      visibility: SymbolVisibility::Hidden,
      binding: SymbolBinding::Local,
      sym_type: SymbolType::NoType,
      version: None,
      demangled_name: None, // Debug symbols don't get demangled
    });
  }

  /// Add source file debug symbol
  pub fn add_source_file_symbol(&mut self, filename: &str) {
    self.add_debug_symbol(N_SO, filename, 0, 0, 0);
  }

  /// Add function debug symbol
  pub fn add_function_debug_symbol(&mut self, func_name: &str, addr: u64) {
    self.add_debug_symbol(N_FUN, func_name, 1, 0, addr);
  }

  /// Add source line debug symbol
  pub fn add_line_number_symbol(&mut self, line: u16, addr: u64) {
    // Line number goes in n_desc
    self.add_debug_symbol(N_SLINE, "", 1, line, addr);

    // Also add to DWARF debug line entries
    self.debug_line.push(DebugLineEntry {
      address: addr,
      file_index: 1, // Default to first file
      line: line as u32,
      column: 0,
      is_stmt: true,
    });
  }

  /// Generate DWARF line number program for debug line entries
  pub fn generate_debug_line_program(&mut self) -> Vec<u8> {
    let mut state = LineNumberProgramState::new();

    state.generate_line_program(&self.debug_line)
  }

  /// Add begin/end scope symbols
  pub fn add_scope_begin_symbol(&mut self, addr: u64) {
    self.add_debug_symbol(N_LBRAC, "", 1, 0, addr);
  }

  pub fn add_scope_end_symbol(&mut self, addr: u64) {
    self.add_debug_symbol(N_RBRAC, "", 1, 0, addr);
  }

  /// Add compiler version debug symbol
  pub fn add_compiler_version_symbol(&mut self, version: &str) {
    self.add_debug_symbol(N_VERSION, version, 0, 0, 0);
  }

  /// Add optimization level debug symbol
  pub fn add_optimization_level_symbol(&mut self, level: u8) {
    self.add_debug_symbol(N_OLEVEL, "", 0, level as u16, 0);
  }

  // ===== DWARF Debug Support =====

  /// Add a string to debug string table and return offset
  pub fn add_debug_string(&mut self, s: &str) -> u32 {
    let offset = self.debug_str.len() as u32;

    self.debug_str.extend_from_slice(s.as_bytes());
    self.debug_str.push(0); // Null terminate

    offset
  }

  /// Add a relocation entry for the text section
  pub fn add_text_relocation(
    &mut self,
    address: u32,
    symbol_name: &str,
    rel_type: ARM64RelocationType,
    is_pcrel: bool,
  ) {
    // Find or create symbol index
    let symbol_index =
      if let Some(&idx) = self.symbol_index_map.get(symbol_name) {
        idx
      } else {
        // Symbol will be resolved later
        let idx =
          (self.external_symbols.len() + self.undefined_symbols.len()) as u32;
        self.symbol_index_map.insert(symbol_name.to_string(), idx);
        idx
      };

    let reloc = RelocationInfo::new(
      address,
      symbol_index,
      true, // External symbol
      rel_type,
      is_pcrel,
      3, // 8 bytes for 64-bit
    );

    self.text_relocations.push(reloc);
  }

  /// Add a relocation entry for the data section
  pub fn add_data_relocation(
    &mut self,
    address: u32,
    symbol_name: &str,
    rel_type: ARM64RelocationType,
  ) {
    // Find or create symbol index
    let symbol_index =
      if let Some(&idx) = self.symbol_index_map.get(symbol_name) {
        idx
      } else {
        let idx =
          (self.external_symbols.len() + self.undefined_symbols.len()) as u32;
        self.symbol_index_map.insert(symbol_name.to_string(), idx);
        idx
      };

    let reloc = RelocationInfo::new(
      address,
      symbol_index,
      true, // External symbol
      rel_type,
      false, // Data relocations typically not PC-relative
      3,     // 8 bytes for 64-bit
    );

    self.data_relocations.push(reloc);
  }

  /// Add a relocation for a branch instruction (BL/B)
  pub fn add_branch_relocation(&mut self, address: u32, target_symbol: &str) {
    self.add_text_relocation(
      address,
      target_symbol,
      ARM64RelocationType::Branch26,
      true, // PC-relative
    );
  }

  /// Add relocations for ADRP + ADD/LDR pair (page-based addressing)
  pub fn add_page_relocation(
    &mut self,
    adrp_address: u32,
    add_address: u32,
    symbol_name: &str,
  ) {
    // ADRP instruction - gets high 21 bits of page
    self.add_text_relocation(
      adrp_address,
      symbol_name,
      ARM64RelocationType::Page21,
      true, // PC-relative
    );

    // ADD/LDR instruction - gets low 12 bits within page
    self.add_text_relocation(
      add_address,
      symbol_name,
      ARM64RelocationType::Pageoff12,
      false, // Not PC-relative
    );
  }

  /// Add GOT (Global Offset Table) relocations
  pub fn add_got_relocation(
    &mut self,
    adrp_address: u32,
    ldr_address: u32,
    symbol_name: &str,
  ) {
    // ADRP to GOT entry
    self.add_text_relocation(
      adrp_address,
      symbol_name,
      ARM64RelocationType::GotLoadPage21,
      true,
    );

    // LDR from GOT entry
    self.add_text_relocation(
      ldr_address,
      symbol_name,
      ARM64RelocationType::GotLoadPageoff12,
      false,
    );
  }

  /// Write relocations to binary format
  fn write_relocations(&self, relocations: &[RelocationInfo]) -> Vec<u8> {
    let mut data = Vec::with_capacity(relocations.len() * 8);

    for reloc in relocations {
      data.extend_from_slice(&reloc.to_bytes());
    }

    data
  }

  /// Add runtime search path (LC_RPATH)
  pub fn add_rpath(&mut self, path: &str) {
    #[repr(C)]
    struct RpathCommand {
      cmd: u32,
      cmdsize: u32,
      path_offset: u32,
    }

    let path_with_null = format!("{path}\0");
    let padded_len = (path_with_null.len() + 7) & !7; // Align to 8 bytes
    let cmd_size = std::mem::size_of::<RpathCommand>() + padded_len;

    let cmd = RpathCommand {
      cmd: LC_RPATH,
      cmdsize: cmd_size as u32,
      path_offset: std::mem::size_of::<RpathCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<RpathCommand>(),
      )
    });

    cmd_data.extend_from_slice(path_with_null.as_bytes());

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add weak dynamic library (LC_LOAD_WEAK_DYLIB)
  pub fn add_weak_dylib(&mut self, name: &str) {
    #[repr(C)]
    struct DylibCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
      timestamp: u32,
      current_version: u32,
      compatibility_version: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmd_size = std::mem::size_of::<DylibCommand>() + padded_len;

    let cmd = DylibCommand {
      cmd: LC_LOAD_WEAK_DYLIB,
      cmdsize: cmd_size as u32,
      name_offset: std::mem::size_of::<DylibCommand>() as u32,
      timestamp: 2,
      current_version: 0x10000,
      compatibility_version: 0x10000,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add function starts (LC_FUNCTION_STARTS).
  #[inline(always)]
  pub fn add_function_starts(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_FUNCTION_STARTS,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Add data in code entries (LC_DATA_IN_CODE).
  #[inline(always)]
  pub fn add_data_in_code(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_DATA_IN_CODE,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Add encryption info for 64-bit (LC_ENCRYPTION_INFO_64).
  #[inline(always)]
  pub fn add_encryption_info_64(
    &mut self,
    crypt_offset: u32,
    crypt_size: u32,
    crypt_id: u32,
  ) {
    #[repr(C)]
    struct EncryptionInfo64Command {
      cmd: u32,
      cmdsize: u32,
      cryptoff: u32,  // File offset of encrypted range
      cryptsize: u32, // File size of encrypted range
      cryptid: u32,   // Which encryption system (0 = not encrypted)
      pad: u32,       // Padding to make 64-bit aligned
    }

    let cmd = EncryptionInfo64Command {
      cmd: LC_ENCRYPTION_INFO_64,
      cmdsize: std::mem::size_of::<EncryptionInfo64Command>() as u32,
      cryptoff: crypt_offset,
      cryptsize: crypt_size,
      cryptid: crypt_id,
      pad: 0,
    };

    self.add_load_command(&cmd);
  }

  /// Add version min macOS (LC_VERSION_MIN_MACOSX)
  #[inline(always)]
  pub fn add_version_min_macos(&mut self, version: u32, sdk: u32) {
    #[repr(C)]
    struct VersionMinCommand {
      cmd: u32,
      cmdsize: u32,
      version: u32,
      sdk: u32,
    }

    let cmd = VersionMinCommand {
      cmd: LC_VERSION_MIN_MACOSX,
      cmdsize: std::mem::size_of::<VersionMinCommand>() as u32,
      version,
      sdk,
    };

    self.add_load_command(&cmd);
  }

  /// Add version min iOS (LC_VERSION_MIN_IPHONEOS).
  #[inline(always)]
  pub fn add_version_min_ios(&mut self, version: u32, sdk: u32) {
    #[repr(C)]
    struct VersionMinCommand {
      cmd: u32,
      cmdsize: u32,
      version: u32,
      sdk: u32,
    }

    let cmd = VersionMinCommand {
      cmd: LC_VERSION_MIN_IPHONEOS,
      cmdsize: std::mem::size_of::<VersionMinCommand>() as u32,
      version,
      sdk,
    };

    self.add_load_command(&cmd);
  }

  /// Add version min tvOS (LC_VERSION_MIN_TVOS).
  #[inline(always)]
  pub fn add_version_min_tvos(&mut self, version: u32, sdk: u32) {
    #[repr(C)]
    struct VersionMinCommand {
      cmd: u32,
      cmdsize: u32,
      version: u32,
      sdk: u32,
    }

    let cmd = VersionMinCommand {
      cmd: LC_VERSION_MIN_TVOS,
      cmdsize: std::mem::size_of::<VersionMinCommand>() as u32,
      version,
      sdk,
    };

    self.add_load_command(&cmd);
  }

  /// Add version min watchOS (LC_VERSION_MIN_WATCHOS)
  pub fn add_version_min_watchos(&mut self, version: u32, sdk: u32) {
    #[repr(C)]
    struct VersionMinCommand {
      cmd: u32,
      cmdsize: u32,
      version: u32,
      sdk: u32,
    }

    let cmd = VersionMinCommand {
      cmd: LC_VERSION_MIN_WATCHOS,
      cmdsize: std::mem::size_of::<VersionMinCommand>() as u32,
      version,
      sdk,
    };

    self.add_load_command(&cmd);
  }

  /// Add linker optimization hint (LC_LINKER_OPTIMIZATION_HINT)
  pub fn add_linker_optimization_hint(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_LINKER_OPTIMIZATION_HINT,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Add dyld exports trie (LC_DYLD_EXPORTS_TRIE)
  pub fn add_dyld_exports_trie(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_DYLD_EXPORTS_TRIE,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Add dyld chained fixups (LC_DYLD_CHAINED_FIXUPS)
  pub fn add_dyld_chained_fixups(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_DYLD_CHAINED_FIXUPS,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Add reexport dylib (LC_REEXPORT_DYLIB)
  pub fn add_reexport_dylib(&mut self, name: &str) {
    #[repr(C)]
    struct DylibCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
      timestamp: u32,
      current_version: u32,
      compatibility_version: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmd_size = std::mem::size_of::<DylibCommand>() + padded_len;

    let cmd = DylibCommand {
      cmd: LC_REEXPORT_DYLIB,
      cmdsize: cmd_size as u32,
      name_offset: std::mem::size_of::<DylibCommand>() as u32,
      timestamp: 2,
      current_version: 0x10000,
      compatibility_version: 0x10000,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add lazy load dylib (LC_LAZY_LOAD_DYLIB)
  pub fn add_lazy_load_dylib(&mut self, name: &str) {
    #[repr(C)]
    struct DylibCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
      timestamp: u32,
      current_version: u32,
      compatibility_version: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmd_size = std::mem::size_of::<DylibCommand>() + padded_len;

    let cmd = DylibCommand {
      cmd: LC_LAZY_LOAD_DYLIB,
      cmdsize: cmd_size as u32,
      name_offset: std::mem::size_of::<DylibCommand>() as u32,
      timestamp: 2,
      current_version: 0x10000,
      compatibility_version: 0x10000,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add upward dylib (LC_LOAD_UPWARD_DYLIB)
  pub fn add_upward_dylib(&mut self, name: &str) {
    #[repr(C)]
    struct DylibCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
      timestamp: u32,
      current_version: u32,
      compatibility_version: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmd_size = std::mem::size_of::<DylibCommand>() + padded_len;

    let cmd = DylibCommand {
      cmd: LC_LOAD_UPWARD_DYLIB,
      cmdsize: cmd_size as u32,
      name_offset: std::mem::size_of::<DylibCommand>() as u32,
      timestamp: 2,
      current_version: 0x10000,
      compatibility_version: 0x10000,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylibCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add dyld info only (LC_DYLD_INFO_ONLY)
  pub fn add_dyld_info_only(&mut self, info: DyldInfo) {
    #[repr(C)]
    struct DyldInfoCommand {
      cmd: u32,
      cmdsize: u32,
      rebase_off: u32,
      rebase_size: u32,
      bind_off: u32,
      bind_size: u32,
      weak_bind_off: u32,
      weak_bind_size: u32,
      lazy_bind_off: u32,
      lazy_bind_size: u32,
      export_off: u32,
      export_size: u32,
    }

    let cmd = DyldInfoCommand {
      cmd: LC_DYLD_INFO_ONLY,
      cmdsize: std::mem::size_of::<DyldInfoCommand>() as u32,
      rebase_off: info.rebase_off,
      rebase_size: info.rebase_size,
      bind_off: info.bind_off,
      bind_size: info.bind_size,
      weak_bind_off: info.weak_bind_off,
      weak_bind_size: info.weak_bind_size,
      lazy_bind_off: info.lazy_bind_off,
      lazy_bind_size: info.lazy_bind_size,
      export_off: info.export_off,
      export_size: info.export_size,
    };

    self.add_load_command(&cmd);
  }

  /// Add linker option (LC_LINKER_OPTION)
  pub fn add_linker_option(&mut self, options: Vec<&str>) {
    #[repr(C)]
    struct LinkerOptionCommand {
      cmd: u32,
      cmdsize: u32,
      count: u32,
    }

    let mut option_data = Vec::new();

    for opt in &options {
      option_data.extend_from_slice(opt.as_bytes());
      option_data.push(0);
    }

    let padded_len = (option_data.len() + 7) & !7;
    let cmd_size = std::mem::size_of::<LinkerOptionCommand>() + padded_len;

    let cmd = LinkerOptionCommand {
      cmd: LC_LINKER_OPTION,
      cmdsize: cmd_size as u32,
      count: options.len() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<LinkerOptionCommand>(),
      )
    });

    cmd_data.extend_from_slice(&option_data);

    while cmd_data.len() < cmd_size {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add LC_CODE_SIGNATURE load command
  pub fn add_code_signature(&mut self, offset: u32, size: u32) {
    let cmd = LinkeditDataCommand {
      cmd: LC_CODE_SIGNATURE,
      cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
      dataoff: offset,
      datasize: size,
    };

    self.add_load_command(&cmd);
  }

  /// Generate ad-hoc code signature with optional requirements and entitlements
  fn generate_code_signature(
    binary_data: &[u8],
    code_limit: u32,
    requirements: Option<&[u8]>,
    entitlements: Option<&str>,
  ) -> Vec<u8> {
    let mut signature = Vec::new();

    // Page size for code signing (16KB on Apple Silicon)
    const PAGE_SIZE: usize = 16384;
    const PAGE_SHIFT: u8 = 14; // log2(16384)

    // Calculate number of code pages (using code_limit, not full binary)
    let n_code_slots = (code_limit as usize).div_ceil(PAGE_SIZE);

    // Build identifier string (bundle ID or binary name)
    let identifier = "com.zo.binary";

    // Calculate how many blobs we'll have
    let mut blob_count = 1u32; // CodeDirectory is always present
    if requirements.is_some() {
      blob_count += 1;
    }
    if entitlements.is_some() {
      blob_count += 1;
    }

    // Calculate sizes for each blob
    let code_dir_size = std::mem::size_of::<CodeDirectory>()
      + identifier.len() + 1  // identifier string
      + n_code_slots * CS_HASH_SIZE_SHA256; // hash slots

    let requirements_size = requirements.map(|r| r.len()).unwrap_or(0);
    let entitlements_size = entitlements
      .map(|e| std::mem::size_of::<EntitlementsBlob>() + e.len())
      .unwrap_or(0);

    let superblob_size = std::mem::size_of::<SuperBlob>()
      + (std::mem::size_of::<BlobIndex>() * blob_count as usize)  // Index entries
      + code_dir_size
      + requirements_size
      + entitlements_size;

    // Write SuperBlob header
    let superblob = SuperBlob {
      magic: CSMAGIC_EMBEDDED_SIGNATURE.to_be(),
      length: (superblob_size as u32).to_be(),
      count: blob_count.to_be(),
    };

    signature.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &superblob as *const _ as *const u8,
        std::mem::size_of::<SuperBlob>(),
      )
    });

    // Calculate offsets for each blob
    let mut current_offset = std::mem::size_of::<SuperBlob>()
      + (std::mem::size_of::<BlobIndex>() * blob_count as usize);

    // Write BlobIndex entries
    let mut blob_indices = Vec::new();

    // Requirements blob (if present)
    if requirements.is_some() {
      blob_indices.push(BlobIndex {
        type_: 2u32.to_be(), // Requirements slot
        offset: (current_offset as u32).to_be(),
      });

      current_offset += requirements_size;
    }

    // Entitlements blob (if present)
    if entitlements.is_some() {
      blob_indices.push(BlobIndex {
        type_: 5u32.to_be(), // Entitlements slot
        offset: (current_offset as u32).to_be(),
      });

      current_offset += entitlements_size;
    }

    // CodeDirectory blob (always last)
    blob_indices.push(BlobIndex {
      type_: 0u32.to_be(), // CodeDirectory slot
      offset: (current_offset as u32).to_be(),
    });

    // Write all blob indices
    for blob_index in &blob_indices {
      signature.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
          blob_index as *const _ as *const u8,
          std::mem::size_of::<BlobIndex>(),
        )
      });
    }

    // Write Requirements blob (if present)
    if let Some(req_data) = requirements {
      // Construct proper RequirementsBlob header
      let req_blob = RequirementsBlob {
        magic: CSMAGIC_REQUIREMENTS.to_be(),
        length: (req_data.len() as u32).to_be(),
        count: 0u32.to_be(), // No individual requirements for ad-hoc signing
      };

      signature.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
          &req_blob as *const _ as *const u8,
          std::mem::size_of::<RequirementsBlob>(),
        )
      });

      signature.extend_from_slice(req_data);
    }

    // Write Entitlements blob (if present)
    if let Some(ent_str) = entitlements {
      let ent_blob = EntitlementsBlob {
        magic: CSMAGIC_ENTITLEMENTS.to_be(),
        length: ((std::mem::size_of::<EntitlementsBlob>() + ent_str.len())
          as u32)
          .to_be(),
      };

      signature.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
          &ent_blob as *const _ as *const u8,
          std::mem::size_of::<EntitlementsBlob>(),
        )
      });

      signature.extend_from_slice(ent_str.as_bytes());
    }

    // Calculate offsets within CodeDirectory
    let hash_offset =
      std::mem::size_of::<CodeDirectory>() + identifier.len() + 1;
    let ident_offset = std::mem::size_of::<CodeDirectory>();

    // Write CodeDirectory
    let code_dir = CodeDirectory {
      magic: CSMAGIC_CODEDIRECTORY.to_be(),
      length: (code_dir_size as u32).to_be(),
      version: 0x20400u32.to_be(), // Version 2.4.0
      flags: CS_ADHOC.to_be() | CS_LINKER_SIGNED.to_be(),
      hash_offset: (hash_offset as u32).to_be(),
      ident_offset: (ident_offset as u32).to_be(),
      n_special_slots: 0,
      n_code_slots: (n_code_slots as u32).to_be(),
      code_limit: code_limit.to_be(),
      hash_size: CS_HASH_SIZE_SHA256 as u8,
      hash_type: CS_HASHTYPE_SHA256,
      platform: 0,
      page_shift: PAGE_SHIFT,
      spare2: 0,
      scatter_offset: 0,
      team_id_offset: 0,
      spare3: 0,
      code_limit_64: (code_limit as u64).to_be(),
      exec_seg_base: 0,
      exec_seg_limit: (code_limit as u64).to_be(),
      exec_seg_flags: 0x01, // Main binary flag
    };

    signature.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &code_dir as *const _ as *const u8,
        std::mem::size_of::<CodeDirectory>(),
      )
    });

    // Write identifier string
    signature.extend_from_slice(identifier.as_bytes());
    signature.push(0); // Null terminator

    // Generate and write code hashes
    for i in 0..n_code_slots {
      let start = i * PAGE_SIZE;
      let end = ((i + 1) * PAGE_SIZE).min(code_limit as usize);
      let page_data = &binary_data[start..end];

      let mut hasher = Sha256::new();

      hasher.update(page_data);

      let hash = hasher.finalize();

      signature.extend_from_slice(&hash);
    }

    // Pad to 16-byte boundary
    while signature.len() % 16 != 0 {
      signature.push(0);
    }

    signature
  }

  /// Create a compilation unit DIE
  pub fn create_compile_unit_die(
    &mut self,
    name: &str,
    producer: &str,
    comp_dir: &str,
    low_pc: u64,
    high_pc: u64,
    language: u16,
  ) -> DebugInfoEntry {
    let name_offset = self.add_debug_string(name);
    let producer_offset = self.add_debug_string(producer);
    let comp_dir_offset = self.add_debug_string(comp_dir);

    DebugInfoEntry {
      tag: DW_TAG_COMPILE_UNIT,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_NAME,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(name_offset),
        },
        DwarfAttribute {
          name: DW_AT_PRODUCER,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(producer_offset),
        },
        DwarfAttribute {
          name: DW_AT_COMP_DIR,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(comp_dir_offset),
        },
        DwarfAttribute {
          name: DW_AT_LOW_PC,
          form: DW_FORM_ADDR,
          value: DwarfValue::Address(low_pc),
        },
        DwarfAttribute {
          name: DW_AT_HIGH_PC,
          form: DW_FORM_DATA8,
          value: DwarfValue::Data8(high_pc - low_pc), // Size
        },
        DwarfAttribute {
          name: DW_AT_LANGUAGE,
          form: DW_FORM_DATA2,
          value: DwarfValue::Data2(language),
        },
        DwarfAttribute {
          name: DW_AT_STMT_LIST,
          form: DW_FORM_SEC_OFFSET,
          value: DwarfValue::SecOffset(0), // Offset to line table
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create a subprogram (function) DIE
  pub fn create_subprogram_die(
    &mut self,
    name: &str,
    low_pc: u64,
    high_pc: u64,
    file_index: u32,
    line: u32,
    is_external: bool,
  ) -> DebugInfoEntry {
    let name_offset = self.add_debug_string(name);

    let mut attributes = vec![
      DwarfAttribute {
        name: DW_AT_NAME,
        form: DW_FORM_STRP,
        value: DwarfValue::StringRef(name_offset),
      },
      DwarfAttribute {
        name: DW_AT_LOW_PC,
        form: DW_FORM_ADDR,
        value: DwarfValue::Address(low_pc),
      },
      DwarfAttribute {
        name: DW_AT_HIGH_PC,
        form: DW_FORM_DATA8,
        value: DwarfValue::Data8(high_pc - low_pc),
      },
      DwarfAttribute {
        name: DW_AT_DECL_FILE,
        form: DW_FORM_DATA4,
        value: DwarfValue::Data4(file_index),
      },
      DwarfAttribute {
        name: DW_AT_DECL_LINE,
        form: DW_FORM_DATA4,
        value: DwarfValue::Data4(line),
      },
    ];

    if is_external {
      attributes.push(DwarfAttribute {
        name: DW_AT_EXTERNAL,
        form: DW_FORM_FLAG_PRESENT,
        value: DwarfValue::Flag(true),
      });
    }

    DebugInfoEntry {
      tag: DW_TAG_SUBPROGRAM,
      attributes,
      children: Vec::new(),
    }
  }

  /// Create a base type DIE (int, float, etc.)
  pub fn create_base_type_die(
    &mut self,
    name: &str,
    byte_size: u8,
    encoding: u8,
  ) -> DebugInfoEntry {
    let name_offset = self.add_debug_string(name);

    DebugInfoEntry {
      tag: DW_TAG_BASE_TYPE,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_NAME,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(name_offset),
        },
        DwarfAttribute {
          name: DW_AT_BYTE_SIZE,
          form: DW_FORM_DATA1,
          value: DwarfValue::Data1(byte_size),
        },
        DwarfAttribute {
          name: DW_AT_ENCODING,
          form: DW_FORM_DATA1,
          value: DwarfValue::Data1(encoding),
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create a DIE for a pointer type
  pub fn create_pointer_type_die(
    &mut self,
    base_type_offset: u32,
  ) -> DebugInfoEntry {
    DebugInfoEntry {
      tag: DW_TAG_POINTER_TYPE,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_TYPE,
          form: DW_FORM_REF4,
          value: DwarfValue::Data4(base_type_offset),
        },
        DwarfAttribute {
          name: DW_AT_BYTE_SIZE,
          form: DW_FORM_DATA1,
          value: DwarfValue::Data1(8), // 64-bit pointer
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create a DIE for a structure type
  pub fn create_structure_type_die(
    &mut self,
    name: &str,
    byte_size: u32,
  ) -> DebugInfoEntry {
    let name_offset = self.add_debug_string(name);

    DebugInfoEntry {
      tag: DW_TAG_STRUCTURE_TYPE,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_NAME,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(name_offset),
        },
        DwarfAttribute {
          name: DW_AT_BYTE_SIZE,
          form: DW_FORM_DATA4,
          value: DwarfValue::Data4(byte_size),
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create a DIE for a structure member
  pub fn create_member_die(
    &mut self,
    name: &str,
    type_offset: u32,
    location: u32,
  ) -> DebugInfoEntry {
    let name_offset = self.add_debug_string(name);

    DebugInfoEntry {
      tag: DW_TAG_MEMBER,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_NAME,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(name_offset),
        },
        DwarfAttribute {
          name: DW_AT_TYPE,
          form: DW_FORM_REF4,
          value: DwarfValue::Data4(type_offset),
        },
        DwarfAttribute {
          name: DW_AT_DATA_MEMBER_LOCATION,
          form: DW_FORM_DATA4,
          value: DwarfValue::Data4(location),
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create a DIE for an array type
  pub fn create_array_type_die(
    &mut self,
    element_type_offset: u32,
    count: u32,
  ) -> DebugInfoEntry {
    let subrange = DebugInfoEntry {
      tag: DW_TAG_SUBRANGE_TYPE,
      attributes: vec![DwarfAttribute {
        name: DW_AT_COUNT,
        form: DW_FORM_DATA4,
        value: DwarfValue::Data4(count),
      }],
      children: Vec::new(),
    };

    DebugInfoEntry {
      tag: DW_TAG_ARRAY_TYPE,
      attributes: vec![DwarfAttribute {
        name: DW_AT_TYPE,
        form: DW_FORM_REF4,
        value: DwarfValue::Data4(element_type_offset),
      }],
      children: vec![subrange],
    }
  }

  /// Create types using different encodings
  pub fn create_boolean_type(&mut self, name: &str) -> DebugInfoEntry {
    self.create_base_type_die(name, 1, DW_ATE_BOOLEAN)
  }

  pub fn create_float_type(&mut self, name: &str, size: u8) -> DebugInfoEntry {
    self.create_base_type_die(name, size, DW_ATE_FLOAT)
  }

  pub fn create_signed_type(&mut self, name: &str, size: u8) -> DebugInfoEntry {
    self.create_base_type_die(name, size, DW_ATE_SIGNED)
  }

  pub fn create_unsigned_type(
    &mut self,
    name: &str,
    size: u8,
  ) -> DebugInfoEntry {
    self.create_base_type_die(name, size, DW_ATE_UNSIGNED)
  }

  pub fn create_utf_type(&mut self, name: &str, size: u8) -> DebugInfoEntry {
    self.create_base_type_die(name, size, DW_ATE_UTF)
  }

  /// Add frame base attribute to a subprogram DIE
  pub fn add_frame_base(
    &mut self,
    die: &mut DebugInfoEntry,
    location_expr: Vec<u8>,
  ) {
    die.attributes.push(DwarfAttribute {
      name: DW_AT_FRAME_BASE,
      form: DW_FORM_BLOCK,
      value: DwarfValue::Block(location_expr),
    });
  }

  /// Add inline string attribute (uses DW_FORM_STRING)
  pub fn add_inline_string_attribute(
    &mut self,
    die: &mut DebugInfoEntry,
    attr_name: u16,
    value: &str,
  ) {
    die.attributes.push(DwarfAttribute {
      name: attr_name,
      form: DW_FORM_STRING,
      value: DwarfValue::String(value.to_string()),
    });
  }

  /// Create a compilation unit with language info
  pub fn create_compilation_unit_with_language(
    &mut self,
    producer: &str,
    language: u16,
  ) -> DebugInfoEntry {
    let producer_offset = self.add_debug_string(producer);

    DebugInfoEntry {
      tag: DW_TAG_COMPILE_UNIT,
      attributes: vec![
        DwarfAttribute {
          name: DW_AT_PRODUCER,
          form: DW_FORM_STRP,
          value: DwarfValue::StringRef(producer_offset),
        },
        DwarfAttribute {
          name: DW_AT_LANGUAGE,
          form: DW_FORM_DATA2,
          value: DwarfValue::Data2(language),
        },
      ],
      children: Vec::new(),
    }
  }

  /// Create Rust compilation unit
  pub fn create_rust_compilation_unit(
    &mut self,
    producer: &str,
  ) -> DebugInfoEntry {
    self.create_compilation_unit_with_language(producer, DW_LANG_RUST)
  }

  /// Create C99 compilation unit
  pub fn create_c99_compilation_unit(
    &mut self,
    producer: &str,
  ) -> DebugInfoEntry {
    self.create_compilation_unit_with_language(producer, DW_LANG_C99)
  }

  /// Create C++14 compilation unit
  pub fn create_cpp14_compilation_unit(
    &mut self,
    producer: &str,
  ) -> DebugInfoEntry {
    self.create_compilation_unit_with_language(producer, DW_LANG_CPP14)
  }

  /// Add a source file for debug info.
  #[inline(always)]
  pub fn add_debug_file(&mut self, filename: &str) -> u32 {
    let index = self.debug_files.len() as u32;

    self.debug_files.push(filename.to_string());

    index
  }

  /// Add a line number entry
  pub fn add_line_number_entry(
    &mut self,
    address: u64,
    file_index: u32,
    line: u32,
    column: u32,
    is_stmt: bool,
  ) {
    self.debug_line.push(DebugLineEntry {
      address,
      file_index,
      line,
      column,
      is_stmt,
    });
  }

  /// Add frame unwind information for a function
  pub fn add_frame_info(
    &mut self,
    start_addr: u64,
    size: u64,
    cfa_instructions: Vec<u8>,
  ) {
    self.debug_frame.push(DebugFrameEntry {
      start_addr,
      size,
      cfa_instructions,
    });
  }

  /// Add a debug frame entry with builder pattern.
  #[inline(always)]
  pub fn add_debug_frame_entry(&mut self, entry: DebugFrameEntry) {
    self.debug_frame.push(entry);
  }

  /// Generate Common Information Entry (CIE) for debug_frame.
  #[inline(always)]
  fn generate_cie(&self) -> Vec<u8> {
    let mut cie = Vec::new();

    // CIE header
    let cie_length: u32 = 20; // Basic CIE length

    cie.extend_from_slice(&cie_length.to_le_bytes()); // Length
    cie.extend_from_slice(&0xffffffffu32.to_le_bytes()); // CIE ID
    cie.push(4); // Version
    cie.push(0); // Augmentation string (empty)

    // Address size and segment size
    cie.push(8); // Address size (8 bytes for 64-bit)
    cie.push(0); // Segment size

    // Code alignment factor (ULEB128)
    cie.push(1); // Code alignment = 1

    // Data alignment factor (SLEB128)
    cie.push(0x78); // Data alignment = -8 (encoded as SLEB128)

    // Return address register
    cie.push(30); // ARM64 link register (x30)

    // Initial instructions for ARM64
    // DW_CFA_def_cfa: r31 (SP) ofs 0
    cie.push(0x0c); // DW_CFA_def_cfa
    cie.push(31); // SP register
    cie.push(0); // Offset 0

    // Padding
    while cie.len() % 8 != 0 {
      cie.push(0x00); // DW_CFA_nop
    }

    cie
  }

  /// Generate debug_frame section data.
  #[inline(always)]
  pub fn generate_debug_frame(&self) -> Vec<u8> {
    if self.debug_frame.is_empty() {
      return Vec::new();
    }

    let mut frame_data = Vec::new();

    // Generate CIE
    let cie_offset = frame_data.len() as u32;
    let cie = self.generate_cie();

    frame_data.extend_from_slice(&cie);

    // Generate FDEs for each function
    for entry in &self.debug_frame {
      let fde = entry.to_fde_bytes(cie_offset);

      frame_data.extend_from_slice(&fde);
    }

    frame_data
  }

  /// Serialize a DIE to binary format
  fn serialize_die(&self, die: &DebugInfoEntry, abbrev_code: u32) -> Vec<u8> {
    let mut data = Vec::new();

    // Write abbreviation code (ULEB128)
    data.extend(&self.encode_uleb128(abbrev_code));

    // Write attributes
    for attr in &die.attributes {
      match &attr.value {
        DwarfValue::Data1(v) => data.push(*v),
        DwarfValue::Data2(v) => data.extend(&v.to_le_bytes()),
        DwarfValue::Data4(v) => data.extend(&v.to_le_bytes()),
        DwarfValue::Data8(v) => data.extend(&v.to_le_bytes()),
        DwarfValue::String(s) => {
          // String should have been converted to StringRef when DIE was created
          // For inline strings, write them directly (though this is rarely
          // used)
          data.push(s.len() as u8);
          data.extend_from_slice(s.as_bytes());
          data.push(0); // Null terminator
        }
        DwarfValue::StringRef(idx) => data.extend(&idx.to_le_bytes()),
        DwarfValue::Address(addr) => data.extend(&addr.to_le_bytes()),
        DwarfValue::Reference(ref_val) => data.extend(&ref_val.to_le_bytes()),
        DwarfValue::SecOffset(off) => data.extend(&off.to_le_bytes()),
        DwarfValue::Flag(b) => data.push(if *b { 1 } else { 0 }),
        DwarfValue::Block(bytes) => {
          data.extend(&self.encode_uleb128(bytes.len() as u32));
          data.extend(bytes);
        }
      }
    }

    // If has children, recursively serialize them
    if !die.children.is_empty() {
      for (i, child) in die.children.iter().enumerate() {
        let child_data =
          self.serialize_die(child, (abbrev_code * 100) + i as u32 + 1);

        data.extend(child_data);
      }
      // End of children marker
      data.push(0);
    }

    data
  }

  /// Encode unsigned value as ULEB128.
  #[inline(always)]
  fn encode_uleb128(&self, mut value: u32) -> Vec<u8> {
    let mut result = Vec::new();

    loop {
      let mut byte = (value & 0x7f) as u8;

      value >>= 7;

      if value != 0 {
        byte |= 0x80;
      }

      result.push(byte);

      if value == 0 {
        break;
      }
    }

    result
  }

  /// Encode signed value as SLEB128.
  #[inline(always)]
  fn encode_sleb128(&self, mut value: i32) -> Vec<u8> {
    let mut result = Vec::new();

    loop {
      let mut byte = (value & 0x7f) as u8;

      value >>= 7;

      let sign_bit = byte & 0x40;
      if (value == 0 && sign_bit == 0) || (value == -1 && sign_bit != 0) {
        result.push(byte);
        break;
      } else {
        byte |= 0x80;

        result.push(byte);
      }
    }
    result
  }

  /// Generate abbreviation table for DIEs
  fn generate_abbrev_table(&self) -> Vec<u8> {
    let mut data = Vec::new();

    // Abbreviation 1: Compile Unit
    data.extend(&self.encode_uleb128(1)); // abbrev code
    data.extend(&self.encode_uleb128(DW_TAG_COMPILE_UNIT as u32));
    data.push(DW_CHILDREN_YES); // has children

    // Attributes for compile unit
    data.extend(&self.encode_uleb128(DW_AT_PRODUCER as u32));
    data.extend(&self.encode_uleb128(DW_FORM_STRP as u32));
    data.extend(&self.encode_uleb128(DW_AT_LANGUAGE as u32));
    data.extend(&self.encode_uleb128(DW_FORM_DATA2 as u32));
    data.extend(&self.encode_uleb128(DW_AT_NAME as u32));
    data.extend(&self.encode_uleb128(DW_FORM_STRP as u32));
    data.extend(&self.encode_uleb128(DW_AT_COMP_DIR as u32));
    data.extend(&self.encode_uleb128(DW_FORM_STRP as u32));
    data.extend(&self.encode_uleb128(DW_AT_LOW_PC as u32));
    data.extend(&self.encode_uleb128(DW_FORM_ADDR as u32));
    data.extend(&self.encode_uleb128(DW_AT_HIGH_PC as u32));
    data.extend(&self.encode_uleb128(DW_FORM_ADDR as u32));
    data.extend(&self.encode_uleb128(DW_AT_STMT_LIST as u32));
    data.extend(&self.encode_uleb128(DW_FORM_SEC_OFFSET as u32));
    data.push(0); // end of attributes
    data.push(0);

    // Abbreviation 2: Subprogram
    data.extend(&self.encode_uleb128(2));
    data.extend(&self.encode_uleb128(DW_TAG_SUBPROGRAM as u32));
    data.push(DW_CHILDREN_NO);

    data.extend(&self.encode_uleb128(DW_AT_NAME as u32));
    data.extend(&self.encode_uleb128(DW_FORM_STRP as u32));
    data.extend(&self.encode_uleb128(DW_AT_LOW_PC as u32));
    data.extend(&self.encode_uleb128(DW_FORM_ADDR as u32));
    data.extend(&self.encode_uleb128(DW_AT_HIGH_PC as u32));
    data.extend(&self.encode_uleb128(DW_FORM_ADDR as u32));
    data.extend(&self.encode_uleb128(DW_AT_TYPE as u32));
    data.extend(&self.encode_uleb128(DW_FORM_REF4 as u32));
    data.extend(&self.encode_uleb128(DW_AT_EXTERNAL as u32));
    data.extend(&self.encode_uleb128(DW_FORM_FLAG as u32));
    data.push(0);
    data.push(0);

    // Abbreviation 3: Variable
    data.extend(&self.encode_uleb128(3));
    data.extend(&self.encode_uleb128(DW_TAG_VARIABLE as u32));
    data.push(DW_CHILDREN_NO);

    data.extend(&self.encode_uleb128(DW_AT_NAME as u32));
    data.extend(&self.encode_uleb128(DW_FORM_STRP as u32));
    data.extend(&self.encode_uleb128(DW_AT_TYPE as u32));
    data.extend(&self.encode_uleb128(DW_FORM_REF4 as u32));
    data.extend(&self.encode_uleb128(DW_AT_LOCATION as u32));
    data.extend(&self.encode_uleb128(DW_FORM_ADDR as u32));
    data.push(0);
    data.push(0);

    // End of abbreviation table
    data.push(0);

    data
  }

  /// Generate line number program
  fn generate_line_program(&self) -> Vec<u8> {
    let mut data = Vec::new();

    // Line number program header
    let header_length_offset = data.len();

    data.extend(&0u32.to_le_bytes()); // placeholder for unit_length
    data.extend(&2u16.to_le_bytes()); // version

    let header_start = data.len();

    data.extend(&0u32.to_le_bytes()); // placeholder for header_length
    data.push(1); // minimum_instruction_length
    data.push(1); // default_is_stmt
    data.push(0xf6u8); // line_base (-10)
    data.push(0x0b); // line_range (11)
    data.push(0x0d); // opcode_base (13)

    // Standard opcode lengths
    data.extend(&[0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1]);

    // Include directories (empty for now)
    data.push(0);

    // File names
    for file in &self.debug_files {
      data.extend(file.as_bytes());
      data.push(0); // null terminator
      data.extend(&self.encode_uleb128(0)); // directory index
      data.extend(&self.encode_uleb128(0)); // modification time
      data.extend(&self.encode_uleb128(0)); // file size
    }

    data.push(0); // end of file names

    // Update header_length
    let header_end = data.len();
    let header_length = (header_end - header_start - 4) as u32;

    data[header_start..header_start + 4]
      .copy_from_slice(&header_length.to_le_bytes());

    // Generate line number program opcodes
    let mut current_address = 0u64;
    let mut current_line = 1u32;
    let mut current_file = 0u32;

    for entry in &self.debug_line {
      // Set file if changed
      if entry.file_index != current_file {
        data.push(0x04); // DW_LNS_set_file
        data.extend(&self.encode_uleb128(entry.file_index + 1)); // 1-based

        current_file = entry.file_index;
      }

      // Set address
      if entry.address != current_address {
        data.push(0); // extended opcode
        data.extend(&self.encode_uleb128(9)); // length
        data.push(0x02); // DW_LNE_set_address
        data.extend(&entry.address.to_le_bytes());

        current_address = entry.address;
      }

      // Advance line
      if entry.line != current_line {
        let line_diff = entry.line as i32 - current_line as i32;

        if (-10..=10).contains(&line_diff) {
          // Use special opcode
          let opcode = ((line_diff + 10) + 13) as u8;

          data.push(opcode);
        } else {
          // Use standard advance_line
          data.push(0x03); // DW_LNS_advance_line
          data.extend(&self.encode_sleb128(line_diff));
          data.push(0x01); // DW_LNS_copy
        }

        current_line = entry.line;
      }

      // Set column if needed
      if entry.column > 0 {
        data.push(0x05); // DW_LNS_set_column
        data.extend(&self.encode_uleb128(entry.column));
      }

      // Set is_stmt if needed
      if entry.is_stmt {
        data.push(0x0b); // DW_LNS_set_prologue_end
      }
    }

    // End sequence
    data.push(0); // extended opcode
    data.extend(&self.encode_uleb128(1));
    data.push(0x01); // DW_LNE_end_sequence

    // Update unit_length
    let total_length = (data.len() - 4) as u32;

    data[header_length_offset..header_length_offset + 4]
      .copy_from_slice(&total_length.to_le_bytes());

    data
  }

  /// Generate debug info section
  fn generate_debug_info(&self) -> Vec<u8> {
    let mut data = Vec::new();

    // Compilation unit header
    let unit_length_offset = data.len();

    data.extend(&0u32.to_le_bytes()); // placeholder for unit_length
    data.extend(&4u16.to_le_bytes()); // version (DWARF 4)
    data.extend(&0u32.to_le_bytes()); // debug_abbrev_offset
    data.push(8); // address_size

    // Serialize all DIEs
    for (i, die) in self.debug_info.iter().enumerate() {
      let die_data = self.serialize_die(die, i as u32 + 1);

      data.extend(die_data);
    }

    // Update unit_length
    let total_length = (data.len() - 4) as u32;

    data[unit_length_offset..unit_length_offset + 4]
      .copy_from_slice(&total_length.to_le_bytes());

    data
  }

  /// Generate debug string table
  fn generate_debug_str(&self) -> Vec<u8> {
    // debug_str is a Vec<u8> already containing the string table
    self.debug_str.clone()
  }

  /// Add __DWARF segment with debug sections
  pub fn add_dwarf_segment(&mut self) {
    if self.debug_info.is_empty() && self.debug_line.is_empty() {
      return; // No debug info to add
    }

    // Generate debug sections
    let debug_info_data = self.generate_debug_info();
    let debug_abbrev_data = self.generate_abbrev_table();
    let debug_str_data = self.generate_debug_str();

    let debug_line_data = if !self.debug_line.is_empty() {
      self.generate_line_program()
    } else {
      Vec::new()
    };

    // Calculate offsets for each section
    let mut current_offset = 0x8000u64; // Start after code/data

    // Create __DWARF segment command
    let mut nsects = 0u32;
    let mut sections = Vec::new();

    // Add __debug_info section
    if !debug_info_data.is_empty() {
      let sect = Section64 {
        sectname: Self::make_cstring("__debug_info"),
        segname: Self::make_cstring("__DWARF"),
        addr: 0, // DWARF sections don't need VM addresses
        size: debug_info_data.len() as u64,
        offset: current_offset as u32,
        align: 0,
        reloff: 0,
        nreloc: 0,
        flags: S_ATTR_DEBUG,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      };

      let len = debug_info_data.len() as u64;

      sections.push((sect, debug_info_data));

      current_offset += len;
      nsects += 1;
    }

    // Add __debug_abbrev section
    if !debug_abbrev_data.is_empty() {
      let sect = Section64 {
        sectname: Self::make_cstring("__debug_abbrev"),
        segname: Self::make_cstring("__DWARF"),
        addr: 0,
        size: debug_abbrev_data.len() as u64,
        offset: current_offset as u32,
        align: 0,
        reloff: 0,
        nreloc: 0,
        flags: S_ATTR_DEBUG,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      };

      let len = debug_abbrev_data.len() as u64;

      sections.push((sect, debug_abbrev_data));

      current_offset += len;
      nsects += 1;
    }

    // Add __debug_str section
    if !debug_str_data.is_empty() {
      let sect = Section64 {
        sectname: Self::make_cstring("__debug_str"),
        segname: Self::make_cstring("__DWARF"),
        addr: 0,
        size: debug_str_data.len() as u64,
        offset: current_offset as u32,
        align: 0,
        reloff: 0,
        nreloc: 0,
        flags: S_ATTR_DEBUG,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      };

      let len = debug_str_data.len() as u64;

      sections.push((sect, debug_str_data));

      current_offset += len;
      nsects += 1;
    }

    // Add __debug_line section
    if !debug_line_data.is_empty() {
      let sect = Section64 {
        sectname: Self::make_cstring("__debug_line"),
        segname: Self::make_cstring("__DWARF"),
        addr: 0,
        size: debug_line_data.len() as u64,
        offset: current_offset as u32,
        align: 0,
        reloff: 0,
        nreloc: 0,
        flags: S_ATTR_DEBUG,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      };

      let len = debug_line_data.len() as u64;

      sections.push((sect, debug_line_data));

      current_offset += len;
      nsects += 1;
    }

    // Add __debug_frame section
    let debug_frame_data = self.generate_debug_frame();
    if !debug_frame_data.is_empty() {
      let sect = Section64 {
        sectname: Self::make_cstring("__debug_frame"),
        segname: Self::make_cstring("__DWARF"),
        addr: 0,
        size: debug_frame_data.len() as u64,
        offset: current_offset as u32,
        align: 0,
        reloff: 0,
        nreloc: 0,
        flags: S_ATTR_DEBUG,
        reserved1: 0,
        reserved2: 0,
        reserved3: 0,
      };

      let len = debug_frame_data.len() as u64;

      sections.push((sect, debug_frame_data));

      current_offset += len;
      nsects += 1;
    }

    // Store sections for later writing
    self.dwarf_sections = sections;

    // Create segment command
    let total_size = current_offset - 0x8000;

    let cmd = SegmentCommand64 {
      cmd: LC_SEGMENT_64,
      cmdsize: std::mem::size_of::<SegmentCommand64>() as u32
        + nsects * std::mem::size_of::<Section64>() as u32,
      segname: Self::make_cstring("__DWARF"),
      vmaddr: 0, // DWARF doesn't need VM address
      vmsize: 0,
      fileoff: 0x8000,
      filesize: total_size,
      maxprot: VM_PROT_READ,
      initprot: VM_PROT_READ,
      nsects,
      flags: 0,
    };

    self.segments.push(cmd);
  }

  /// Update section with relocation information.
  #[inline(always)]
  fn update_section_relocations(&mut self) {
    // Find __text section and update with relocations
    for section in &mut self.sections {
      if section.sectname == Self::make_cstring("__text") {
        if !self.text_relocations.is_empty() {
          // Relocations will be written after LINKEDIT data
          // For now, just mark that we have relocations
          section.nreloc = self.text_relocations.len() as u32;
          // reloff will be set during finish()
        }
      } else if section.sectname == Self::make_cstring("__data")
        && !self.data_relocations.is_empty()
      {
        section.nreloc = self.data_relocations.len() as u32;
        // reloff will be set during finish()
      }
    }
  }

  /// Add thread local init function pointers.
  #[inline(always)]
  pub fn add_thread_local_init_funcs(&mut self, funcs: Vec<u64>) {
    let data = funcs
      .iter()
      .flat_map(|ptr| ptr.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__thread_init"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_THREAD_LOCAL_INIT_FUNCTION_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add coalesced section.
  #[inline(always)]
  pub fn add_coalesced_section(&mut self, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__textcoal_nt"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 4,
      reloff: 0,
      nreloc: 0,
      flags: S_COALESCED,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.code.extend_from_slice(&data);
  }

  /// Add large (>= 4GB) zerofill section.
  #[inline(always)]
  pub fn add_gb_zerofill_section(&mut self, size: u64) {
    let section = Section64 {
      sectname: Self::make_cstring("__huge_bss"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size,
      offset: 0, // Zerofill
      align: 12, // 2^12 = 4KB alignment for huge sections
      reloff: 0,
      nreloc: 0,
      flags: S_GB_ZEROFILL,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
  }

  /// Add interposing section for function interposition.
  #[inline(always)]
  pub fn add_interposing_section(&mut self, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__interpose"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_INTERPOSING,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add DTrace DOF section.
  #[inline(always)]
  pub fn add_dtrace_dof_section(&mut self, dof_data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__dof"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: dof_data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_DTRACE_DOF,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.code.extend_from_slice(&dof_data);
  }

  /// Set header flags for weak symbols.
  #[inline(always)]
  pub fn set_weak_defines(&mut self) {
    self.header.flags |= MH_WEAK_DEFINES;
  }

  /// Set header flags for binding to weak symbols.
  #[inline(always)]
  pub fn set_binds_to_weak(&mut self) {
    self.header.flags |= MH_BINDS_TO_WEAK;
  }

  /// Set header flags for root safe.
  #[inline(always)]
  pub fn set_root_safe(&mut self) {
    self.header.flags |= MH_ROOT_SAFE;
  }

  /// Set header flags for setuid safe.
  #[inline(always)]
  pub fn set_setuid_safe(&mut self) {
    self.header.flags |= MH_SETUID_SAFE;
  }

  /// Set header flags for no reexported dylibs.
  #[inline(always)]
  pub fn set_no_reexported_dylibs(&mut self) {
    self.header.flags |= MH_NO_REEXPORTED_DYLIBS;
  }

  /// Add ID for dynamic linker (LC_ID_DYLINKER)
  pub fn add_dylinker_id(&mut self, path: &str) {
    #[repr(C)]
    struct DylinkerCommand {
      cmd: u32,
      cmdsize: u32,
      name_offset: u32,
    }

    let path_with_null = format!("{path}\0");
    let padded_len = (path_with_null.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<DylinkerCommand>() + padded_len;

    let cmd = DylinkerCommand {
      cmd: LC_ID_DYLINKER,
      cmdsize: cmdsize as u32,
      name_offset: std::mem::size_of::<DylinkerCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<DylinkerCommand>(),
      )
    });

    cmd_data.extend_from_slice(path_with_null.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add sub-framework (LC_SUB_FRAMEWORK)
  pub fn add_sub_framework(&mut self, name: &str) {
    #[repr(C)]
    struct SubFrameworkCommand {
      cmd: u32,
      cmdsize: u32,
      umbrella_offset: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<SubFrameworkCommand>() + padded_len;

    let cmd = SubFrameworkCommand {
      cmd: LC_SUB_FRAMEWORK,
      cmdsize: cmdsize as u32,
      umbrella_offset: std::mem::size_of::<SubFrameworkCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<SubFrameworkCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add sub-umbrella (LC_SUB_UMBRELLA)
  pub fn add_sub_umbrella(&mut self, name: &str) {
    #[repr(C)]
    struct SubUmbrellaCommand {
      cmd: u32,
      cmdsize: u32,
      sub_umbrella_offset: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<SubUmbrellaCommand>() + padded_len;

    let cmd = SubUmbrellaCommand {
      cmd: LC_SUB_UMBRELLA,
      cmdsize: cmdsize as u32,
      sub_umbrella_offset: std::mem::size_of::<SubUmbrellaCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<SubUmbrellaCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add sub-client (LC_SUB_CLIENT)
  pub fn add_sub_client(&mut self, name: &str) {
    #[repr(C)]
    struct SubClientCommand {
      cmd: u32,
      cmdsize: u32,
      client_offset: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<SubClientCommand>() + padded_len;

    let cmd = SubClientCommand {
      cmd: LC_SUB_CLIENT,
      cmdsize: cmdsize as u32,
      client_offset: std::mem::size_of::<SubClientCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<SubClientCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add sub-library (LC_SUB_LIBRARY)
  pub fn add_sub_library(&mut self, name: &str) {
    #[repr(C)]
    struct SubLibraryCommand {
      cmd: u32,
      cmdsize: u32,
      sub_library_offset: u32,
    }

    let name_with_null = format!("{name}\0");
    let padded_len = (name_with_null.len() + 7) & !7;
    let cmdsize = std::mem::size_of::<SubLibraryCommand>() + padded_len;

    let cmd = SubLibraryCommand {
      cmd: LC_SUB_LIBRARY,
      cmdsize: cmdsize as u32,
      sub_library_offset: std::mem::size_of::<SubLibraryCommand>() as u32,
    };

    let mut cmd_data = Vec::new();

    cmd_data.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &cmd as *const _ as *const u8,
        std::mem::size_of::<SubLibraryCommand>(),
      )
    });

    cmd_data.extend_from_slice(name_with_null.as_bytes());

    while cmd_data.len() < cmdsize {
      cmd_data.push(0);
    }

    self.add_load_command(&cmd_data);
  }

  /// Add section with no dead stripping.
  #[inline(always)]
  pub fn add_no_dead_strip_section(&mut self, name: &str, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring(name),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_REGULAR | S_ATTR_NO_DEAD_STRIP,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };

    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add self-modifying code section.
  #[inline(always)]
  pub fn add_self_modifying_code_section(&mut self, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring("__smc"),
      segname: Self::make_cstring("__TEXT"),
      addr: TEXT_VM_ADDR + self.code.len() as u64,
      size: data.len() as u64,
      offset: (CODE_OFFSET as usize + self.code.len()) as u32,
      align: 4,
      reloff: 0,
      nreloc: 0,
      flags: S_REGULAR | S_ATTR_SELF_MODIFYING_CODE | S_ATTR_SOME_INSTRUCTIONS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };
    self.sections.push(section);
    self.code.extend_from_slice(&data);
  }

  /// Add section with live support.
  #[inline(always)]
  pub fn add_live_support_section(&mut self, name: &str, data: Vec<u8>) {
    let section = Section64 {
      sectname: Self::make_cstring(name),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_REGULAR | S_ATTR_LIVE_SUPPORT,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };
    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Add lazy dylib symbol pointers.
  #[inline(always)]
  pub fn add_lazy_dylib_symbol_ptrs(&mut self, ptrs: Vec<u64>) {
    let data = ptrs
      .iter()
      .flat_map(|ptr| ptr.to_le_bytes())
      .collect::<Vec<_>>();

    let section = Section64 {
      sectname: Self::make_cstring("__la_dylib_ptr"),
      segname: Self::make_cstring("__DATA"),
      addr: DATA_VM_ADDR + self.data.len() as u64,
      size: data.len() as u64,
      offset: (DATA_FILE_OFFSET + self.data.len() as u64) as u32,
      align: 3,
      reloff: 0,
      nreloc: 0,
      flags: S_LAZY_DYLIB_SYMBOL_POINTERS,
      reserved1: 0,
      reserved2: 0,
      reserved3: 0,
    };
    self.sections.push(section);
    self.data.extend_from_slice(&data);
  }

  /// Finalizes the Mach-O binary and returns the complete executable
  ///
  /// This method assembles all segments and load commands into a valid Mach-O
  /// file
  ///
  /// # Returns
  /// A vector containing the complete Mach-O executable bytes
  pub fn finish(self) -> Vec<u8> {
    self.finish_internal(false)
  }

  /// Internal finish implementation that handles code signing
  fn finish_internal(mut self, with_signature: bool) -> Vec<u8> {
    let mut output = Vec::new();

    // Update sections with relocation information
    self.update_section_relocations();

    // Build symbol index if not already done
    if self.symbol_index_map.is_empty()
      && (!self.local_symbols.is_empty()
        || !self.external_symbols.is_empty()
        || !self.undefined_symbols.is_empty())
    {
      self.build_symbol_index();
    }

    // Prepare symbol tables
    let n_local = self.local_symbols.len() as u32;
    let n_external = self.external_symbols.len() as u32;
    let n_undefined = self.undefined_symbols.len() as u32;

    // Move symbols out of self to avoid cloning
    let mut all_symbols = Vec::with_capacity(
      self.local_symbols.len()
        + self.external_symbols.len()
        + self.undefined_symbols.len(),
    );

    all_symbols.append(&mut self.local_symbols);
    all_symbols.append(&mut self.external_symbols);
    all_symbols.append(&mut self.undefined_symbols);

    let n_symbols = all_symbols.len() as u32;

    // Build string table
    let mut string_table = vec![0u8]; // Start with null byte
    let mut symbol_string_offsets = Vec::new();

    for symbol in &all_symbols {
      let offset = string_table.len() as u32;

      // Use demangled name if available, otherwise use regular name
      let name_to_use = symbol.demangled_name.as_ref().unwrap_or(&symbol.name);
      string_table.extend_from_slice(name_to_use.as_bytes());

      // Add version suffix if present (e.g., "@1.0.0")
      if let Some(ref version) = symbol.version {
        string_table.push(b'@');
        string_table.extend_from_slice(version.as_bytes());
      }

      string_table.push(0); // Null terminate
      symbol_string_offsets.push(offset);
    }

    // Calculate LINKEDIT content offsets and sizes
    let linkedit_start = LINKEDIT_FILE_OFFSET as u32;
    let symtab_offset = linkedit_start;
    let symtab_size = n_symbols * std::mem::size_of::<Nlist64>() as u32;
    let strtab_offset = symtab_offset + symtab_size;
    let strtab_size = string_table.len() as u32;
    let indirect_symtab_offset = strtab_offset + strtab_size;
    let indirect_symtab_size = self.indirect_symbols.len() as u32 * 4;

    // Calculate base LINKEDIT content size
    let base_linkedit_size = symtab_size + strtab_size + indirect_symtab_size;

    // Calculate signature size if needed
    let mut signature_size = 0u32;
    let mut signature_offset = 0u32;

    if with_signature {
      // Signature starts right after the base LINKEDIT content
      signature_offset = linkedit_start + base_linkedit_size;

      // Calculate the size of the binary WITHOUT signature (for code_limit)
      let code_limit = LINKEDIT_FILE_OFFSET + base_linkedit_size as u64;

      // Calculate signature size
      const PAGE_SIZE: usize = 16384;
      let n_code_pages = (code_limit as usize).div_ceil(PAGE_SIZE);
      let identifier = "com.zo.binary";

      signature_size = (std::mem::size_of::<SuperBlob>()
        + std::mem::size_of::<BlobIndex>()
        + std::mem::size_of::<CodeDirectory>()
        + identifier.len()
        + 1
        + n_code_pages * CS_HASH_SIZE_SHA256) as u32;

      // Align to 16 bytes
      signature_size = (signature_size + 15) & !15;
    }

    // Total LINKEDIT size includes signature
    let total_linkedit_size = base_linkedit_size + signature_size;

    // Add required load commands if not already added
    if n_symbols > 0 {
      self.add_symtab(symtab_offset, n_symbols, strtab_offset, strtab_size);
      self.add_dysymtab(
        n_local,
        n_external,
        n_undefined,
        indirect_symtab_offset,
        self.indirect_symbols.len() as u32,
      );
    }

    // Add LINKEDIT segment with total size (including signature)
    self.add_linkedit_segment(linkedit_start, total_linkedit_size);

    // Add entry point if set
    if let Some(entry_offset) = self.entry_point {
      self.add_main(entry_offset);
    }

    // Calculate sizes
    let mut sizeofcmds = 0u32;
    let mut commands = Vec::new();

    // First add segment commands
    for segment in &self.segments {
      let mut cmd_data = unsafe {
        std::slice::from_raw_parts(
          segment as *const _ as *const u8,
          std::mem::size_of::<SegmentCommand64>(),
        )
        .to_vec()
      };

      // Add sections for this segment
      if segment.nsects > 0 {
        for section in &self.sections {
          if section.segname == segment.segname {
            cmd_data.extend_from_slice(unsafe {
              std::slice::from_raw_parts(
                section as *const _ as *const u8,
                std::mem::size_of::<Section64>(),
              )
            });
          }
        }
      }

      sizeofcmds += cmd_data.len() as u32;

      commands.push(cmd_data);
    }

    // Add the size of all load commands from the buffer
    sizeofcmds += self.load_commands_buf.len() as u32;

    // Count the actual number of load commands
    let load_command_count = self.load_command_offsets.len() as u32;

    // Add LC_CODE_SIGNATURE if needed
    let mut signature_cmd_data = Vec::new();
    if with_signature {
      let sig_cmd = LinkeditDataCommand {
        cmd: LC_CODE_SIGNATURE,
        cmdsize: std::mem::size_of::<LinkeditDataCommand>() as u32,
        dataoff: signature_offset,
        datasize: signature_size,
      };

      signature_cmd_data = unsafe {
        std::slice::from_raw_parts(
          &sig_cmd as *const _ as *const u8,
          std::mem::size_of::<LinkeditDataCommand>(),
        )
        .to_vec()
      };

      sizeofcmds += signature_cmd_data.len() as u32;
    }

    // Update header with correct counts
    self.header.ncmds = commands.len() as u32 + load_command_count + if with_signature { 1 } else { 0 };
    self.header.sizeofcmds = sizeofcmds;

    // Write header
    output.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &self.header as *const _ as *const u8,
        std::mem::size_of::<MachHeader64>(),
      )
    });

    // Write segment load commands first
    for cmd in &commands {
      output.extend_from_slice(cmd);
    }

    // Write all other load commands from the buffer
    output.extend_from_slice(&self.load_commands_buf);

    // Write signature command if present
    if !signature_cmd_data.is_empty() {
      output.extend_from_slice(&signature_cmd_data);
    }

    // Pad to code offset
    while output.len() < CODE_OFFSET as usize {
      output.push(0);
    }

    // Write code section in TEXT segment
    output.extend_from_slice(&self.code);

    // Pad to next page for DATA segment
    while output.len() < DATA_FILE_OFFSET as usize {
      output.push(0);
    }

    // Write data section
    if !self.data.is_empty() {
      output.extend_from_slice(&self.data);
    }

    // Write non-lazy symbol pointers if present
    if !self.indirect_symbols.is_empty() {
      for _ in &self.indirect_symbols {
        // Initialize with zeros (will be resolved by dyld)
        output.extend_from_slice(&[0u8; 8]);
      }
    }

    // Pad to LINKEDIT offset
    while output.len() < LINKEDIT_FILE_OFFSET as usize {
      output.push(0);
    }

    // Write symbol table
    for (symbol, str_offset) in
      all_symbols.iter().zip(symbol_string_offsets.iter())
    {
      // Apply visibility and binding modifiers to n_type and n_desc
      let mut final_n_type = symbol.n_type;
      let mut final_n_desc = symbol.n_desc;

      // Apply visibility flags
      match symbol.visibility {
        SymbolVisibility::Hidden => {
          final_n_desc |= N_WEAK_DEF;
        }
        SymbolVisibility::PrivateExternal => {
          final_n_type |= N_PEXT;
        }
        _ => {}
      }

      // Apply binding flags
      match symbol.binding {
        SymbolBinding::Weak => {
          final_n_desc |= N_WEAK_REF;
        }
        SymbolBinding::Global => {
          final_n_type |= N_EXT;
        }
        _ => {}
      }

      // Set symbol type specific flags
      match symbol.sym_type {
        SymbolType::Tls => {
          // Thread-local storage symbols
          final_n_desc |= 0x0400; // N_TLS flag (implementation specific)
        }
        SymbolType::Indirect => {
          final_n_type = (final_n_type & !N_TYPE) | N_INDR;
        }
        _ => {}
      }

      let nlist = Nlist64 {
        n_strx: *str_offset,
        n_type: final_n_type,
        n_sect: symbol.n_sect,
        n_desc: final_n_desc,
        n_value: symbol.n_value,
      };

      output.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
          &nlist as *const _ as *const u8,
          std::mem::size_of::<Nlist64>(),
        )
      });
    }

    // Write string table
    output.extend_from_slice(&string_table);

    // Write indirect symbol table
    // Combine indirect_symbols with symbols from symbol_refs
    let mut all_indirect = self.indirect_symbols.clone();

    if !self.symbol_refs.is_empty() {
      let refs_indirect = self.generate_indirect_symbol_table();

      for idx in refs_indirect {
        if !all_indirect.contains(&idx) {
          all_indirect.push(idx);
        }
      }
    }

    for &index in &all_indirect {
      output.extend_from_slice(&index.to_le_bytes());
    }

    // Write relocations (if any)
    let mut reloc_offset = output.len() as u32;

    // Write text relocations
    if !self.text_relocations.is_empty() {
      // Update text section reloff
      for section in &mut self.sections {
        if section.sectname == Self::make_cstring("__text") {
          section.reloff = reloc_offset;
          break;
        }
      }
      let text_reloc_data = self.write_relocations(&self.text_relocations);

      output.extend_from_slice(&text_reloc_data);

      reloc_offset = output.len() as u32;
    }

    // Write data relocations
    if !self.data_relocations.is_empty() {
      // Update data section reloff
      for section in &mut self.sections {
        if section.sectname == Self::make_cstring("__data") {
          section.reloff = reloc_offset;
          break;
        }
      }

      let data_reloc_data = self.write_relocations(&self.data_relocations);

      output.extend_from_slice(&data_reloc_data);
    }

    // Pad to complete LINKEDIT segment size (without signature)
    let base_final_size =
      (LINKEDIT_FILE_OFFSET + base_linkedit_size as u64) as usize;

    while output.len() < base_final_size {
      output.push(0);
    }

    // Add code signature if requested
    if with_signature {
      // The code_limit is the size WITHOUT the signature
      let code_limit = output.len() as u32;

      let signature = Self::generate_code_signature(
        &output,
        code_limit,
        self.requirements.as_deref(),
        self.entitlements.as_deref(),
      );

      output.extend_from_slice(&signature);
    }

    output
  }

  /// Finalize with ad-hoc code signature
  pub fn finish_with_signature(self) -> Vec<u8> {
    self.finish_internal(true)
  }
}
impl Default for MachO {
  fn default() -> Self {
    Self::new()
  }
}

/// Builder for creating universal/fat binaries
#[derive(Debug)]
pub struct UniversalBinary {
  /// Architectures to include /* (cpu_type, cpu_subtype, binary_data) */
  architectures: Vec<(u32, u32, Vec<u8>)>,
  /// Whether to use 64-bit fat format
  use_64bit: bool,
}
impl UniversalBinary {
  /// Create a new universal binary builder
  pub fn new() -> Self {
    Self {
      architectures: Vec::new(),
      use_64bit: false,
    }
  }

  /// Create a new 64-bit universal binary builder
  pub fn new_64bit() -> Self {
    Self {
      architectures: Vec::new(),
      use_64bit: true,
    }
  }

  /// Add an architecture to the universal binary
  pub fn add_architecture(
    &mut self,
    cpu_type: u32,
    cpu_subtype: u32,
    binary_data: Vec<u8>,
  ) {
    self
      .architectures
      .push((cpu_type, cpu_subtype, binary_data));
  }

  /// Add an ARM64 architecture
  pub fn add_arm64(&mut self, binary_data: Vec<u8>) {
    self.add_architecture(CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64_ALL, binary_data);
  }

  /// Add an x86_64 architecture
  pub fn add_x86_64(&mut self, binary_data: Vec<u8>) {
    self.add_architecture(CPU_TYPE_X86_64, CPU_SUBTYPE_X86_64_ALL, binary_data);
  }

  /// Create a universal binary from ARM64 and x86_64 binaries
  pub fn create_universal(
    arm64_binary: Vec<u8>,
    x86_64_binary: Vec<u8>,
  ) -> Vec<u8> {
    let mut builder = Self::new();
    builder.add_arm64(arm64_binary);
    builder.add_x86_64(x86_64_binary);
    builder.build()
  }

  /// Build the universal binary
  pub fn build(self) -> Vec<u8> {
    if self.architectures.is_empty() {
      panic!("No architectures added to universal binary");
    }

    let mut output = Vec::new();
    let arch_count = self.architectures.len() as u32;

    // Calculate header size
    let header_size = if self.use_64bit {
      std::mem::size_of::<FatHeader>()
        + std::mem::size_of::<FatArch64>() * self.architectures.len()
    } else {
      std::mem::size_of::<FatHeader>()
        + std::mem::size_of::<FatArch>() * self.architectures.len()
    };

    // Write fat header
    let fat_header = FatHeader {
      magic: if self.use_64bit {
        FAT_MAGIC_64.to_be()
      } else {
        FAT_MAGIC.to_be()
      },
      nfat_arch: arch_count.to_be(),
    };

    output.extend_from_slice(unsafe {
      std::slice::from_raw_parts(
        &fat_header as *const _ as *const u8,
        std::mem::size_of::<FatHeader>(),
      )
    });

    // Calculate offsets and alignments for each architecture
    let mut current_offset = header_size;
    let mut arch_entries = Vec::new();

    for (cpu_type, cpu_subtype, binary_data) in &self.architectures {
      // Align offset to page boundary (4KB)
      let alignment = 12u32; // 2^12 = 4096
      let alignment_bytes = 1usize << alignment;

      current_offset =
        (current_offset + alignment_bytes - 1) & !(alignment_bytes - 1);

      arch_entries.push((
        *cpu_type,
        *cpu_subtype,
        current_offset,
        binary_data.len(),
        alignment,
      ));

      current_offset += binary_data.len();
    }

    // Write architecture descriptors
    if self.use_64bit {
      for (cpu_type, cpu_subtype, offset, size, align) in &arch_entries {
        let fat_arch = FatArch64 {
          cputype: cpu_type.to_be(),
          cpusubtype: cpu_subtype.to_be(),
          offset: (*offset as u64).to_be(),
          size: (*size as u64).to_be(),
          align: align.to_be(),
          reserved: 0,
        };

        output.extend_from_slice(unsafe {
          std::slice::from_raw_parts(
            &fat_arch as *const _ as *const u8,
            std::mem::size_of::<FatArch64>(),
          )
        });
      }
    } else {
      for (cpu_type, cpu_subtype, offset, size, align) in &arch_entries {
        let fat_arch = FatArch {
          cputype: cpu_type.to_be(),
          cpusubtype: cpu_subtype.to_be(),
          offset: (*offset as u32).to_be(),
          size: (*size as u32).to_be(),
          align: align.to_be(),
        };

        output.extend_from_slice(unsafe {
          std::slice::from_raw_parts(
            &fat_arch as *const _ as *const u8,
            std::mem::size_of::<FatArch>(),
          )
        });
      }
    }

    // Write each architecture's binary data at the calculated offset
    for ((_, _, offset, _, _), (_, _, binary_data)) in
      arch_entries.iter().zip(&self.architectures)
    {
      // Pad to alignment
      while output.len() < *offset {
        output.push(0);
      }

      // Write binary data
      output.extend_from_slice(binary_data);
    }

    output
  }
}
impl Default for UniversalBinary {
  fn default() -> Self {
    Self::new()
  }
}
