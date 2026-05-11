mod macho;

#[cfg(test)]
mod tests;

pub use macho::{
  ARM64RelocationType, CODE_OFFSET, DATA_SEGMENT_INDEX, DebugFrameEntry,
  EXECUTABLE_PATH_PREFIX, LIBSYSTEM_DYLIB_ORDINAL, MachO, PAGE_MASK,
  RAYLIB_NAME_MAP, SEGMENT_ALIGN, SymbolVisibility, TEXT_SECTION_BASE,
  UniversalBinary, VM_BASE, ZO_RUNTIME_SYMBOL_PREFIX, raylib_c_name,
  round_up_segment,
};
