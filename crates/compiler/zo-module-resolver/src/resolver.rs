use zo_interner::{Interner, Symbol};

use rustc_hash::FxHashMap as HashMap;

use std::path::{Path, PathBuf};

/// Re-interns a symbol from one interner into another.
pub fn translate_symbol(
  src: Symbol,
  src_interner: &Interner,
  dst_interner: &mut Interner,
) -> Symbol {
  let name = src_interner.get(src);
  dst_interner.intern(name)
}

/// Result of resolving a module path to a file.
pub struct ResolvedModule {
  /// Filesystem path to the .zo file.
  pub path: PathBuf,
  /// File contents.
  pub source: String,
  /// If resolution used the selective import fallback,
  /// this is the last path segment (the symbol to import).
  pub selective_symbol: Option<String>,
}

/// Maps module paths (e.g., `core::math`) to filesystem files.
///
/// Resolution is a simple filesystem lookup — no graph algorithms,
/// no dependency solving. First match wins.
pub struct ModuleResolver {
  /// Ordered search paths: [std_lib_path, project_src_path, ...]
  search_paths: Vec<PathBuf>,
  /// Cache: stringified module path -> resolved source.
  cache: HashMap<String, ResolvedModule>,
}

impl ModuleResolver {
  /// Creates a new [`ModuleResolver`] with the given search paths.
  pub fn new(search_paths: Vec<PathBuf>) -> Self {
    Self {
      search_paths,
      cache: HashMap::default(),
    }
  }

  /// The search-path roots used for module resolution.
  /// Exposed so callers (zo-compiler's implicit-pack rule)
  /// can tell "directly under a root" from "nested in a
  /// sub-folder namespace".
  pub fn search_paths(&self) -> &[PathBuf] {
    &self.search_paths
  }

  /// Resolves a module path (e.g., `["core", "math"]`) to a file.
  ///
  /// A zo project has exactly ONE `lib.zo` at its root; folders
  /// beneath are namespaces (no nested `lib.zo`). So resolution
  /// looks for a `.zo` file at the joined segment path:
  ///   `{search_path}/{segments joined by /}.zo`
  ///
  /// Glob loads over a folder namespace (`load foo::*;` where
  /// `foo/` is a directory) are handled by the caller through
  /// [`resolve_folder_entries`]; the per-file resolver here
  /// only deals with single .zo modules.
  ///
  /// For `load core::math;` with core search path `/lib/core/`:
  /// tries `/lib/core/core/math.zo`. Since the search path
  /// already points to the `core` root, the first segment
  /// `core` maps to the search path itself — handled by
  /// checking if the first segment matches the search path's
  /// directory name and skipping it.
  pub fn resolve(
    &mut self,
    segments: &[Symbol],
    interner: &zo_interner::Interner,
  ) -> Option<&ResolvedModule> {
    let key = self.cache_key(segments, interner);

    if self.cache.contains_key(&key) {
      return self.cache.get(&key);
    }

    let names = segments
      .iter()
      .map(|s| interner.get(*s))
      .collect::<Vec<_>>();

    for search_path in &self.search_paths {
      // Try direct path: {search_path}/{seg0}/{seg1}/...zo
      if let Some(resolved) = Self::try_resolve(search_path, &names, None) {
        self.cache.insert(key.clone(), resolved);

        return self.cache.get(&key);
      }

      // Try skipping first segment if it matches the search
      // path directory name.
      if names.len() > 1
        && let Some(dir_name) = search_path.file_name()
        && dir_name == names[0]
        && let Some(resolved) =
          Self::try_resolve(search_path, &names[1..], None)
      {
        self.cache.insert(key.clone(), resolved);

        return self.cache.get(&key);
      }

      // Selective import fallback: `load foo::bar;` where
      // `bar` is a symbol inside `foo.zo`, not a submodule.
      // Try resolving with all-but-last segment as the module.
      if names.len() > 1 {
        let last = names.last().unwrap().to_string();
        let parent = &names[..names.len() - 1];

        if let Some(resolved) =
          Self::try_resolve(search_path, parent, Some(last.clone()))
        {
          self.cache.insert(key.clone(), resolved);

          return self.cache.get(&key);
        }

        // Also try with dir-name skip for selective imports.
        if !parent.is_empty()
          && let Some(dir_name) = search_path.file_name()
          && dir_name == parent[0]
          && let Some(resolved) =
            Self::try_resolve(search_path, &parent[1..], Some(last))
        {
          self.cache.insert(key.clone(), resolved);

          return self.cache.get(&key);
        }
      }
    }

    None
  }

  /// Tries to resolve segments relative to a base path —
  /// `base/seg0/.../segN.zo`. Folders are namespaces and have
  /// no `lib.zo` body; folder enumeration for `::*` loads is
  /// driven by [`Self::resolve_folder_entries`].
  fn try_resolve(
    base: &Path,
    names: &[&str],
    selective: Option<String>,
  ) -> Option<ResolvedModule> {
    if names.is_empty() {
      return None;
    }

    let mut file_path = base.to_path_buf();

    for name in names {
      file_path.push(name);
    }

    let zo_path = file_path.with_extension("zo");

    if zo_path.is_file() {
      return Self::read_module(&zo_path, selective);
    }

    None
  }

  /// Enumerates the `.zo` files inside a folder namespace.
  /// Used by the compiler to expand `load foo::*;` when `foo`
  /// is a directory. Returns sorted absolute paths so the
  /// load order is stable across runs.
  pub fn resolve_folder_entries(
    &self,
    segments: &[Symbol],
    interner: &zo_interner::Interner,
  ) -> Option<Vec<PathBuf>> {
    if segments.is_empty() {
      return None;
    }

    let names = segments
      .iter()
      .map(|s| interner.get(*s))
      .collect::<Vec<_>>();

    for search_path in &self.search_paths {
      if let Some(folder) = Self::folder_for(search_path, &names) {
        return Some(Self::collect_zo_files(&folder));
      }

      if names.len() > 1
        && let Some(dir_name) = search_path.file_name()
        && dir_name == names[0]
        && let Some(folder) = Self::folder_for(search_path, &names[1..])
      {
        return Some(Self::collect_zo_files(&folder));
      }
    }

    None
  }

  fn folder_for(base: &Path, names: &[&str]) -> Option<PathBuf> {
    if names.is_empty() {
      return None;
    }

    let mut path = base.to_path_buf();

    for name in names {
      path.push(name);
    }

    if path.is_dir() { Some(path) } else { None }
  }

  fn collect_zo_files(folder: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
      .into_iter()
      .flatten()
      .flatten()
      .map(|e| e.path())
      .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "zo"))
      .collect();

    entries.sort();
    entries
  }

  /// Reads a .zo file into a ResolvedModule.
  fn read_module(
    path: &Path,
    selective_symbol: Option<String>,
  ) -> Option<ResolvedModule> {
    std::fs::read_to_string(path)
      .ok()
      .map(|source| ResolvedModule {
        path: path.to_path_buf(),
        source,
        selective_symbol,
      })
  }

  /// Builds a cache key from interned symbols.
  fn cache_key(
    &self,
    segments: &[Symbol],
    interner: &zo_interner::Interner,
  ) -> String {
    segments
      .iter()
      .map(|s| interner.get(*s))
      .collect::<Vec<_>>()
      .join("::")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use zo_interner::Interner;

  fn core_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../compiler-lib/core")
  }

  #[test]
  fn test_resolve_core_math() {
    let mut interner = Interner::new();
    let core_sym = interner.intern("core");
    let math_sym = interner.intern("math");

    let mut resolver = ModuleResolver::new(vec![core_path()]);
    let result = resolver.resolve(&[core_sym, math_sym], &interner);

    assert!(result.is_some(), "should resolve core::math");

    let module = result.unwrap();

    assert!(module.path.ends_with("math.zo"));
    assert!(module.source.contains("PI"));
  }

  #[test]
  fn test_resolve_core_io() {
    let mut interner = Interner::new();
    let core_sym = interner.intern("core");
    let io_sym = interner.intern("io");

    let mut resolver = ModuleResolver::new(vec![core_path()]);
    let result = resolver.resolve(&[core_sym, io_sym], &interner);

    assert!(result.is_some(), "should resolve core::io");

    let module = result.unwrap();

    assert!(module.path.ends_with("io.zo"));
    assert!(module.source.contains("showln"));
  }

  #[test]
  fn test_resolve_nonexistent() {
    let mut interner = Interner::new();
    let core_sym = interner.intern("core");
    let nope_sym = interner.intern("nope");

    let mut resolver = ModuleResolver::new(vec![core_path()]);
    let result = resolver.resolve(&[core_sym, nope_sym], &interner);

    assert!(result.is_none(), "should not resolve core::nope");
  }

  #[test]
  fn test_resolve_caches() {
    let mut interner = Interner::new();
    let core_sym = interner.intern("core");
    let math_sym = interner.intern("math");

    let mut resolver = ModuleResolver::new(vec![core_path()]);

    // First call.
    let r1 = resolver.resolve(&[core_sym, math_sym], &interner);
    assert!(r1.is_some());
    let path1 = r1.unwrap().path.clone();

    // Second call hits cache.
    let r2 = resolver.resolve(&[core_sym, math_sym], &interner);
    assert!(r2.is_some());
    assert_eq!(path1, r2.unwrap().path);
  }

  #[test]
  fn test_resolve_directory_module() {
    let mut interner = Interner::new();
    let core_sym = interner.intern("core");
    let num_sym = interner.intern("num");

    let mut resolver = ModuleResolver::new(vec![core_path()]);

    // core::num should resolve to num/lib.zo if it exists,
    // or fail if it doesn't.
    let result = resolver.resolve(&[core_sym, num_sym], &interner);

    // `resolve` only finds single .zo files; folder
    // namespaces are enumerated through
    // `resolve_folder_entries`, not this entry point.
    assert!(
      result.is_none(),
      "core::num has no num.zo file, resolve should return None"
    );
  }
}
