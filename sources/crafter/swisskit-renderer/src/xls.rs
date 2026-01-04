//! XLS/XLSX parser for preview rendering.
//!
//! Parses Excel files into structured data for table rendering.
//! Supports both legacy `.xls` and modern `.xlsx` formats via `calamine`.

mod data;
mod parser;

pub use data::XlsData;
pub use parser::*;
