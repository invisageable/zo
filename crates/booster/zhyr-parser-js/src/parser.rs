#![allow(dead_code)]

use zhyr_ast::ast::{
  Ast, Dir, File, FileKind, Item, ItemKind, Module, ModuleKind,
};

use zhoo_session::session::Session;

use zo_core::interner::Interner;
use zo_core::Result;

use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_parser::{StringInput, Syntax};

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
      Ok(source) => {
        let filename = path.display().to_string();
        let source_map: Lrc<SourceMap> = Default::default();
        let comments = SingleThreadedComments::default();

        let source_file = source_map
          .new_source_file(swc_common::FileName::Custom(filename), source);

        let mut parser = swc_ecma_parser::Parser::new(
          Syntax::Es(Default::default()),
          StringInput::from(&*source_file),
          Some(&comments),
        );

        match parser.parse_module() {
          Ok(module) => {
            println!("{module:?}");

            Ok(Item {
              kind: ItemKind::File(File {
                kind: FileKind::Module(Module {
                  kind: ModuleKind::Js(module),
                }),
              }),
            })
          }
          Err(error) => panic!("{error:?}"),
        }
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
