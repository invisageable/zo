mod index;
mod position;
mod server;

use server::ZoLanguageServer;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
  env_logger::init();

  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let (service, socket) =
    LspService::new(|client| ZoLanguageServer::new(client));

  Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
  use std::path::Path;
  use zo_compiler::Compiler;

  #[test]
  fn trace_cross_file_goto_def() {
    let source = "load core::math;\n\nfun main() {\n  imu x: int = pow(2, 10);\n  showln(\"{x}\");\n}\n";
    let path = Path::new("/tmp/lsp_debug.zo");
    std::fs::write(path, source).unwrap();

    let mut search = zo_compiler::default_core_search_paths();
    search.push(path.parent().unwrap().to_path_buf());
    eprintln!("=== search_paths ===");
    for p in &search {
      eprintln!("  {} (exists={})", p.display(), p.exists());
    }
    let mut compiler = Compiler::with_search_paths(search);
    let (semantic, _, parsing, session) = compiler.analyze_source(source, path);

    eprintln!("=== funs ({}) ===", semantic.funs.len());
    for fun in &semantic.funs {
      let name = session.interner.get(fun.name);
      let pack = fun.owning_pack.map(|p| session.interner.get(p).to_string());
      eprintln!("  {:<20} owning_pack={:<15?} span={}", name, pack, fun.span);
    }

    eprintln!("\n=== pack_paths ({}) ===", semantic.pack_paths.len());
    for (sym, p) in &semantic.pack_paths {
      let name = session.interner.get(*sym);
      eprintln!("  {:<20} -> {}", name, p.display());
    }

    eprintln!("\n=== use_def_map ({}) ===", semantic.use_def_map.len());

    // Check pow
    let pow_fun = semantic
      .funs
      .iter()
      .find(|f| session.interner.get(f.name) == "pow");
    eprintln!("\n=== pow ===");
    match pow_fun {
      Some(f) => {
        let pack = f.owning_pack.map(|p| session.interner.get(p).to_string());
        eprintln!("  span={} owning_pack={:?}", f.span, pack);
        if let Some(ps) = f.owning_pack {
          match semantic.pack_paths.get(&ps) {
            Some(p) => eprintln!("  resolved: {}", p.display()),
            None => eprintln!("  NOT IN pack_paths"),
          }
        }
      }
      None => eprintln!("  NOT IN funs"),
    }

    // Verify tree
    let pow_off = source.find("pow(").unwrap() as u32;
    eprintln!("\n=== tree at offset {} ===", pow_off);
    let mut best = None;
    let mut smallest = u16::MAX;
    for (idx, span) in parsing.tree.spans.iter().enumerate() {
      if pow_off >= span.start && pow_off < span.end() && span.len < smallest {
        smallest = span.len;
        best = Some(idx);
      }
    }
    if let Some(idx) = best {
      let span = parsing.tree.spans[idx];
      eprintln!(
        "  idx={} token={:?} span={}",
        idx, parsing.tree.nodes[idx].token, span
      );
      if let Some(val) = parsing.tree.value(idx as u32) {
        eprintln!("  value={:?}", val);
      }
    }

    assert!(pow_fun.is_some(), "pow must be in funs");
  }
}

#[cfg(test)]
mod tests2 {
  use zo_compiler::Compiler;

  #[test]
  fn trace_search_paths() {
    let compiler = Compiler::new();
    eprintln!("=== search_paths ===");
    for p in compiler.search_paths() {
      eprintln!("  {} (exists={})", p.display(), p.exists());
    }
  }
}
