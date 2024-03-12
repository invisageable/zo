/// file system.

pub trait Fs<P>: Sized
where
  P: Into<std::path::PathBuf>,
{
  fn read(&self, path: P) -> std::io::Result<String>;
  fn read_to_string(&self, path: P) -> std::io::Result<String>;

  fn make_dir(&self, path: P) -> std::io::Result<()>;
  fn read_dir(&self, path: P) -> std::io::Result<Vec<std::path::PathBuf>>;

  fn make_file(&self, path: P, bytes: impl AsRef<[u8]>) -> std::io::Result<()>;
  fn read_file(&self, path: P) -> std::io::Result<String>;
}

#[derive(Debug)]
pub struct FileSystem;

impl<P> Fs<P> for FileSystem
where
  P: Into<std::path::PathBuf>,
{
  fn read(&self, _path: P) -> std::io::Result<String> {
    todo!()
  }

  fn read_to_string(&self, path: P) -> std::io::Result<String> {
    std::fs::read_to_string(path.into())
  }

  fn make_dir(&self, path: P) -> std::io::Result<()> {
    let path = path.into();

    if path.is_dir() {
      return Ok(());
    }

    std::fs::create_dir_all(path)
  }

  fn read_dir(&self, path: P) -> std::io::Result<Vec<std::path::PathBuf>> {
    std::fs::read_dir(path.into())?
      .map(|ok_dir| Ok(ok_dir?.path()))
      .collect()
  }

  fn make_file(&self, path: P, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    use std::io::Write;

    std::fs::File::create(path.into())
      .map(|mut file| file.write_all(bytes.as_ref()))
      .unwrap()
  }

  fn read_file(&self, path: P) -> std::io::Result<String> {
    use std::io::Read;

    std::fs::File::open(path.into())
      .map(|f| {
        let mut buffer = String::with_capacity(f.metadata()?.len() as usize);

        std::io::BufReader::new(f).read_to_string(&mut buffer)?;

        Ok(buffer)
      })
      .unwrap()
  }
}
