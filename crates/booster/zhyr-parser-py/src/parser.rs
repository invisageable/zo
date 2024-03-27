#![allow(dead_code)]

use zhyr_ast::ast::{
  Ast, Dir, File, FileKind, Item, ItemKind, Module, ModuleKind,
};

use zhoo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::Result;

use rustpython_parser::Mode;

struct Parser<'paths> {
  interner: &'paths Interner,
}

impl<'paths> Parser<'paths> {
  #[inline]
  fn new(interner: &'paths Interner) -> Self {
    Self { interner }
  }

  fn parse(&mut self, paths: &Vec<std::path::PathBuf>) -> Result<Ast> {
    let mut ast = Ast::new();

    for path in paths {
      ast.add_item(self.parse_item(path)?);
    }

    Ok(ast)
  }

  fn parse_item(&mut self, path: &std::path::PathBuf) -> Result<Item> {
    match path {
      path if path.is_dir() => self.parse_item_dir(path),
      path if path.is_file() => self.parse_item_file(path),
      _ => panic!(),
    }
  }

  fn parse_item_dir(&mut self, path: &std::path::PathBuf) -> Result<Item> {
    Ok(Item {
      kind: ItemKind::Dir(Dir(path.into())),
    })
  }

  fn parse_item_file(&mut self, path: &std::path::PathBuf) -> Result<Item> {
    match std::fs::read_to_string(path) {
      Ok(source) => self.parse_module(path, &source),
      Err(error) => panic!("{error}"),
    }
  }

  fn parse_module(
    &mut self,
    path: &std::path::Path,
    source: &str,
  ) -> Result<Item> {
    let source_path = path.display().to_string();

    let result_module =
      rustpython_parser::parse(source, Mode::Module, &source_path);

    match result_module {
      Ok(module) => {
        println!("{module:?}");

        Ok(Item {
          kind: ItemKind::File(File {
            kind: FileKind::Module(Module {
              kind: ModuleKind::Py(module),
            }),
          }),
        })
      }
      Err(error) => panic!("{error}"),
    }
  }
}

/// ## examples.
///
/// ```
/// ```
pub fn parse(
  session: &mut Session,
  paths: &Vec<std::path::PathBuf>,
) -> Result<Ast> {
  Parser::new(&session.interner).parse(paths)
}
