//! Convention checks the executor runs as declarations execute.
//!
//! [`Checker`] pilots every individual checker (naming today). The
//! executor owns one and forwards each declared name to it, so check
//! implementations live here instead of inside the execution loop.
//! Checks report through `zo-reporter`'s warning channel — they never
//! stop compilation.

pub mod checker;

pub use checker::Checker;

#[cfg(test)]
mod tests;
