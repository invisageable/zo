// its my first try writing a module system. It is not a workable system, the
// system does not understand public and privacy and also the relationship
// between parent-child packages. Some improvements will be patch. We'll also
// need to move/create a scope at the session level to avoid name clash. Also
// there are too many `session.to_owned()` due to the `Packer` implementation.
//
// not sure for that one but maybe we'll need petgraph.

use zo_reader::reader;
use zo_reporter::Result;
use zo_session::session::Session;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

/// The representation of the pack system.
pub struct Packer {
  /// A list of packs.
  packs: std::collections::HashMap<String, String>,
}

impl Packer {
  /// Creates a new pack system.
  #[inline(always)]
  pub fn new(// reporter: &'aa mut Reporter
  ) -> Self {
    Self {
      packs: std::collections::HashMap::with_capacity(0usize),
    }
  }

  /// Adds a source pack into the pack map.
  #[inline]
  pub fn add_pack(&mut self, name: String, source: String) {
    self.packs.insert(name, source);
  }

  /// Compiles packs.
  pub fn packs(&self) {
    // for (name, source) in &self.packs {
    // println!("Name = {name}.\n\n{source}");
    // }
  }
}

impl Default for Packer {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// Retrieves all zo files from a path.
fn collect_files(pathdir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
  let mut files = Vec::with_capacity(0usize);

  if pathdir.is_dir() {
    if let Ok(entries) = std::fs::read_dir(pathdir) {
      for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
          files.extend(collect_files(&path)?);
        } else if path.extension().map_or(false, |ext| ext == "zo") {
          files.push(path);
        }
      }
    }
  } else {
    // we assume we deal with a file.
    files.push(pathdir.to_path_buf());
  }

  Ok(files)
}

/// Loads packs files from an url dir path.
pub fn load_packs_from_pathdir(
  session: &Session,
  pathdir: &str,
) -> Result<std::collections::HashMap<String, String>> {
  let session = std::sync::Arc::new(std::sync::Mutex::new(session));

  Ok(
    collect_files(std::path::Path::new(pathdir))?
      .par_iter()
      .map(|pathname| {
        let mut session = session.lock().unwrap().to_owned(); // todo(ivs) — to many `to_owned()`.
        let source = reader::read_file_from_path(&mut session, pathname)
          .expect("read file");
        let pathfile =
          pathname.file_stem().unwrap().to_str().unwrap().to_string();
        (pathfile, source)
      })
      .collect(),
  )
}

/// Packs all zo files into a hashmap.
#[inline]
pub fn pack(
  mut session: Session,
  pathdir: impl ToString,
) -> Result<std::collections::HashMap<String, String>> {
  let mut packsys = Packer::new();
  // for (name, source) in
  //   load_packs_from_pathdir(&session, "crates/compiler-library")?
  // {
  //   packsys.add_pack(name, source);
  // }

  packsys.packs.insert(
    "entry".into(),
    reader::read_file_from_path(&mut session, pathdir.to_string())?,
  );

  Ok(packsys.packs)
}
