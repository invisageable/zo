mod error;

pub use error::{Error, ErrorKind};

pub type Result<T> = anyhow::Result<T, Vec<Error>>;
