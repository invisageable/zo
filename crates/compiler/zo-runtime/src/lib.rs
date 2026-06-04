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
