//! This module is specific for my way to implement a `tokenizer` or a `parser`.
//! For example `opcase` can be strictly different for another compiler.
//!
//! #### examples.
//!
//! ```
//! use swisskit::case::bitcase::lowercase;
//! use swisskit::case::strcase;
//!
//! assert!(lowercase::is_lowercase(b'e'));
//! ```

pub mod bitcase;
pub mod charcase;
pub mod macros;
pub mod strcase;
