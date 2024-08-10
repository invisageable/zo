//! adapted from the work of @ravern.
//! @see https://github.com/ravern/typical.
//!
//! This module implements the Hindley-Milner type inference.
//!
//! Actually it is a not working system.

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod env;
pub mod inferencer;
pub mod scheme;
pub mod subst;
pub mod supply;
pub mod unifier;
