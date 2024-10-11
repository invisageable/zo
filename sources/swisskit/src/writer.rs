/// Creates a new directory from a pathdir.
///
/// #### params.
///
/// | param   | description                |
/// |---------|----------------------------|
/// | pathdir | The path of the directory. |
///
/// #### examples.
///
/// ```ignore
/// use swisskit::writer::make_dir;
///
/// make_dir("./path/to/dir").unwrap();
/// ```
///
/// note — if the file already exist, the program will not crash.
#[inline]
pub fn make_dir(pathdir: impl AsRef<std::path::Path>) -> std::io::Result<()> {
  if pathdir.as_ref().is_dir() {
    return Ok(());
  }

  std::fs::create_dir_all(pathdir)
}

/// Creates a filled file at a specific location from a pathname and bytes.
///
/// #### params.
///
/// |          |                          |
/// |----------|--------------------------|
/// | pathname | The name of the file.    |
/// | bytes    | The source code as bytes |
///
/// #### examples.
///
/// ```ignore
/// use swisskit::writer::make_file;
///
/// make_file("my-name.txt", b"my file content.").unwrap();
/// ```
#[inline]
pub fn make_file(
  pathname: impl ToString,
  bytes: impl AsRef<[u8]>,
) -> std::io::Result<()> {
  use std::io::Write;

  std::fs::File::create(pathname.to_string())
    .and_then(|mut file| file.write_all(bytes.as_ref()))
}
