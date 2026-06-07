//! On-disk bundling for mobile targets.
//!
//! Turns a linked, ad-hoc-signed Mach-O binary plus the platform
//! runtime dylib into a runnable app container — an `.app` directory
//! for iOS. The compiler calls this after linking for a mobile
//! `--target`, the same place desktop builds stage the `deps/`
//! runtime dylib.

pub mod ios;
