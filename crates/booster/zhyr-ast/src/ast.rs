#![allow(dead_code)]

use smol_str::SmolStr;

#[derive(Clone, Debug, Default)]
pub struct Project {
  pub assets: Vec<Asset>,
}

impl Project {
  // no allocation.
  #[inline]
  pub fn new() -> Self {
    Self {
      assets: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn add_asset(&mut self, asset: Asset) {
    self.assets.push(asset);
  }
}

#[derive(Clone, Debug)]
pub struct Asset {
  pub kind: AssetKind,
}

#[derive(Clone, Debug)]
pub enum AssetKind {
  Dir(Dir),
  File(File),
}

#[derive(Clone, Debug)]
pub struct Module {
  pub kind: ModuleKind,
}

#[derive(Clone, Debug)]
pub enum ModuleKind {
  Js(swc_ecma_ast::Module),
  Py(rustpython_ast::Mod),
}

#[derive(Clone, Debug)]
pub struct Dir(pub std::path::PathBuf);

#[derive(Clone, Debug)]
pub struct File {
  pub kind: FileKind,
}

#[derive(Clone, Debug)]
pub enum FileKind {
  /// a text plain file — it can be a `.txt`, `.js`, etc.
  Module(Module),
  /// an image file — `.png`, `.webp`, etc.
  Image(Image),
  /// a svg file.
  Svg(Svg),
  /// a format video — `.mp4`, `.mpeg`, etc.
  Video(Video),
  /// a text plain file — it can be a `.txt`, etc.
  TextPlain(Source),
}

#[derive(Clone, Debug)]
pub struct Image;

#[derive(Clone, Debug)]
pub struct Svg;

#[derive(Clone, Debug)]
pub struct Video;

#[derive(Clone, Debug)]
pub struct Source {
  pub path: std::path::PathBuf,
  pub source: SmolStr,
}

#[derive(Clone, Debug, Default)]
pub struct Ast {
  pub items: Vec<Item>,
}

impl Ast {
  // no allocation.
  #[inline]
  pub fn new() -> Self {
    Self {
      items: Vec::with_capacity(0usize),
    }
  }

  #[inline]
  pub fn add_item(&mut self, item: Item) {
    self.items.push(item);
  }
}

#[derive(Clone, Debug)]
pub struct Item {
  pub kind: ItemKind,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
  Dir(Dir),
  File(File),
}
