pub mod exports;
pub mod resolver;

#[cfg(test)]
mod tests;

pub use exports::{
  AbstractDef, AbstractImpl, AbstractMethod, ExportedComponentBody,
  ExportedConst, ExportedEnum, ExportedGenericBody, ExportedLiteral,
  ExportedStruct, ExportedTreeSlice, ExportedVar, ImportedSymbols,
  ModuleExports, ModuleHarvest, Slot, SplicedComponentBody, SplicedGenericBody,
  extract_exports, splice_component_bodies, splice_generic_bodies,
};
pub use resolver::{ModuleResolver, ResolvedModule, translate_symbol};
