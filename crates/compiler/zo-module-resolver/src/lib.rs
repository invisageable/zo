pub mod exports;
pub mod resolver;

pub use exports::{ExportedVar, ModuleExports, extract_exports};
pub use resolver::{ModuleResolver, ResolvedModule, translate_symbol};
