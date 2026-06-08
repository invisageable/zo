// The graphical / webview dispatcher pulls in the entire GPU +
// webview dependency tree; gate it behind `ui` so lean builds
// (`--no-default-features`) compile the core runtime alone.
#[cfg(feature = "ui")]
mod runtime;

pub mod arr;
pub mod assert;
pub mod base64;
pub mod bufio;
pub mod channel;
pub mod ctxsw;
pub mod env;
pub mod file;
pub mod hash;
pub mod io;
pub mod map;
pub mod mem;
pub mod net;
pub mod os;
pub mod pool;
pub mod process;
pub mod regex;
pub mod scheduler;
pub mod select;
pub mod spike;
pub mod stack;
pub mod str;
pub mod sys;
pub mod task;
pub mod test;
pub mod time;
pub mod tls;
pub mod vec;

#[cfg(feature = "ui")]
pub use runtime::Runtime;

/// The static-bundle web server, for `zo run --target web`. Lives in
/// the web runtime; the desktop backends (and thus this re-export) are
/// gated off iOS, where the UIKit binary never reaches the host.
#[cfg(all(feature = "ui", not(target_os = "ios")))]
pub use zo_runtime_web::{Browsering, Server};

/// Force-link the iOS UIKit backend's `_zo_run_native` into this
/// cdylib. The desktop dispatcher references `zo-runtime-native`,
/// which co-locates and thereby keeps its entry symbol; on iOS the
/// dispatcher is a no-op, so the entry the AOT binary calls is dead-
/// stripped unless something references the crate. This re-export is
/// that reference.
#[cfg(all(feature = "ui", target_os = "ios"))]
pub use zo_runtime_ios::zo_run_native;
