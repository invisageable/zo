//! Process exit codes, mapped to failure modes so a script can tell
//! what went wrong from `$?`. Parse / usage errors exit `2` through
//! clap's own default, which `EXIT_CODE_USAGE` mirrors for the manual
//! argument checks.

/// The success exit code.
pub const EXIT_CODE_SUCCESS: i32 = 0;
/// A compilation or diagnostic error — the program is ill-formed. The
/// generic failure code for the compile path.
pub const EXIT_CODE_ERROR: i32 = 1;
/// A usage error — bad or missing command-line arguments. Matches the
/// code clap returns for its own parse failures.
pub const EXIT_CODE_USAGE: i32 = 2;
/// An IO error — a source file is missing or unreadable.
pub const EXIT_CODE_IO: i32 = 3;
/// A link, bundle, or packaging failure — assembling the output
/// artifact failed after a clean compile.
pub const EXIT_CODE_BUNDLE: i32 = 4;
