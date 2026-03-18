use zo_interner::{Interner, Symbol};

use std::collections::HashMap;
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

/// Maps module paths (e.g., `std::math`) to filesystem files.
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
      cache: HashMap::new(),
    }
  }

  /// Resolves a module path (e.g., `["std", "math"]`) to a file.
  ///
  /// Search order per search path:
  /// 1. `{search_path}/{segments joined by /}.zo` (file module)
  /// 2. `{search_path}/{segments joined by /}/lib.zo` (dir module)
  ///
  /// For `load std::math;` with std search path `/lib/std/`:
  /// 1. tries `/lib/std/std/math.zo`
  /// 2. tries `/lib/std/std/math/lib.zo`
  ///
  /// But since the std search path already points to the std
  /// root, the first segment `std` maps to the search path
  /// itself. So we handle this by checking if the first segment
  /// matches the search path's directory name and skip it.
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

  /// Tries to resolve segments relative to a base path.
  fn try_resolve(
    base: &Path,
    names: &[&str],
    selective: Option<String>,
  ) -> Option<ResolvedModule> {
    if names.is_empty() {
      return None;
    }

    // Build path from segments.
    let mut file_path = base.to_path_buf();

    for name in names {
      file_path.push(name);
    }

    // Try as file: base/seg0/seg1.zo
    let zo_path = file_path.with_extension("zo");

    if zo_path.is_file() {
      return Self::read_module(&zo_path, selective);
    }

    // Try as directory module: base/seg0/seg1/lib.zo
    let lib_path = file_path.join("lib.zo");

    if lib_path.is_file() {
      return Self::read_module(&lib_path, selective);
    }

    None
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

  fn std_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../compiler-lib/std")
  }

  #[test]
  fn test_resolve_std_math() {
    let mut interner = Interner::new();
    let std_sym = interner.intern("std");
    let math_sym = interner.intern("math");

    let mut resolver = ModuleResolver::new(vec![std_path()]);
    let result = resolver.resolve(&[std_sym, math_sym], &interner);

    assert!(result.is_some(), "should resolve std::math");

    let module = result.unwrap();

    assert!(module.path.ends_with("math.zo"));
    assert!(module.source.contains("PI"));
  }

  #[test]
  fn test_resolve_std_io() {
    let mut interner = Interner::new();
    let std_sym = interner.intern("std");
    let io_sym = interner.intern("io");

    let mut resolver = ModuleResolver::new(vec![std_path()]);
    let result = resolver.resolve(&[std_sym, io_sym], &interner);

    assert!(result.is_some(), "should resolve std::io");

    let module = result.unwrap();

    assert!(module.path.ends_with("io.zo"));
    assert!(module.source.contains("showln"));
  }

  #[test]
  fn test_resolve_nonexistent() {
    let mut interner = Interner::new();
    let std_sym = interner.intern("std");
    let nope_sym = interner.intern("nope");

    let mut resolver = ModuleResolver::new(vec![std_path()]);
    let result = resolver.resolve(&[std_sym, nope_sym], &interner);

    assert!(result.is_none(), "should not resolve std::nope");
  }

  #[test]
  fn test_resolve_caches() {
    let mut interner = Interner::new();
    let std_sym = interner.intern("std");
    let math_sym = interner.intern("math");

    let mut resolver = ModuleResolver::new(vec![std_path()]);

    // First call.
    let r1 = resolver.resolve(&[std_sym, math_sym], &interner);
    assert!(r1.is_some());
    let path1 = r1.unwrap().path.clone();

    // Second call hits cache.
    let r2 = resolver.resolve(&[std_sym, math_sym], &interner);
    assert!(r2.is_some());
    assert_eq!(path1, r2.unwrap().path);
  }

  #[test]
  fn test_resolve_directory_module() {
    let mut interner = Interner::new();
    let std_sym = interner.intern("std");
    let num_sym = interner.intern("num");

    let mut resolver = ModuleResolver::new(vec![std_path()]);

    // std::num should resolve to num/lib.zo if it exists,
    // or fail if it doesn't.
    let result = resolver.resolve(&[std_sym, num_sym], &interner);

    // num/ exists as a directory but has no lib.zo — only
    // int.zo and u8.zo. So this should not resolve.
    assert!(
      result.is_none(),
      "std::num has no lib.zo, should not resolve"
    );
  }
}
