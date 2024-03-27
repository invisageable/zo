#![allow(dead_code)]

use zhoo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::Result;

use smol_str::ToSmolStr;
use walkdir::{DirEntry, WalkDir};

pub fn cargo_ws_root() -> String {
  let program = env!("CARGO");

  let output = std::process::Command::new(program)
    .args(["locate-project", "--workspace", "--message-format=plain"])
    .output()
    .unwrap()
    .stdout;

  let cargo_path = std::path::Path::new(match std::str::from_utf8(&output) {
    Ok(path) => path.trim(),
    Err(error) => panic!("{error}"),
  });

  cargo_path
    .parent()
    .unwrap()
    .to_path_buf()
    .display()
    .to_string()
}

struct Reader<'source> {
  interner: &'source mut Interner,
}

impl<'source> Reader<'source> {
  #[inline]
  fn new(interner: &'source mut Interner) -> Self {
    Self { interner }
  }

  fn read(&mut self, pathname: &str) -> Result<Vec<std::path::PathBuf>> {
    // let root = std::path::Path::new(pathname);
    // let pathname = format!("{}/{pathname}", cargo_ws_root());

    // println!("\n{pathname}\n");

    let root = std::path::Path::new(pathname);
    let walkdir = WalkDir::new(root);
    let mut paths = Vec::with_capacity(0usize); // mo allocation.

    for result_entry in walkdir {
      match result_entry {
        Ok(entry) => self.read_entry(root, &mut paths, entry),
        Err(error) => panic!("{error}"),
      }
    }

    println!("{paths:?}");

    Ok(paths)
  }

  fn read_entry(
    &mut self,
    root: &std::path::Path,
    paths: &mut Vec<std::path::PathBuf>,
    entry: DirEntry,
  ) {
    let path = entry.path();

    if path.is_dir() {
      let result_folder = path.strip_prefix(root);

      match result_folder {
        Ok(foldername) => {
          let dirname = foldername.display();

          self.interner.intern(&dirname.to_smolstr());
          paths.push(foldername.into());
          println!("{foldername:?}");
        }
        Err(error) => panic!("{error}"),
      };
    } else if path.is_file() {
      let pathname = path.display();

      self.interner.intern(&pathname.to_smolstr());
      paths.push(path.into());
      println!("{pathname}");
    }
  }
}

/// ## examples.
///
/// ```
/// ```
pub fn read(session: &mut Session) -> Result<Vec<std::path::PathBuf>> {
  Reader::new(&mut session.interner).read(&session.settings.input)
}
