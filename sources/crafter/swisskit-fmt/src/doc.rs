//! Conversion of the papers — Strictly Pretty by [Christian Lindig][0]
//! released the March 6, 2000.
//!
//! [0]: https://lindig.github.io/papers/strictly-pretty-2000.pdf.
//!
//! The conversion has been started the August 26 2024 by @invisageable.

/// The representation of a document data type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Doc {
  /// An empty document.
  #[default]
  Nil,
  /// A concatenation of two documents.
  Cons(Box<Doc>, Box<Doc>),
  /// A string within a document.
  Text(String),
  /// ...
  Nest(i32, Box<Doc>),
  /// A line break within a document.
  Brk,
  /// A group in conjunction with the optional line breaks.
  Group(Box<Doc>),
}
impl Doc {
  /// Creates an empty document.
  #[inline(always)]
  pub const fn nil() -> Self {
    Self::Nil
  }

  /// Creates a document from two documents.
  #[inline(always)]
  pub fn cons(lhs: Doc, rhs: Doc) -> Self {
    Self::Cons(Box::new(lhs), Box::new(rhs))
  }

  /// Creates a text in a document.
  #[inline(always)]
  pub const fn text(s: String) -> Self {
    Self::Text(s)
  }

  /// Creates a nested document from indentation.
  #[inline(always)]
  pub fn nest(indent: i32, doc: Doc) -> Self {
    Self::Nest(indent, Box::new(doc))
  }

  /// Creates a line break within a document.
  #[inline(always)]
  pub const fn brk() -> Self {
    Self::Brk
  }

  #[inline(always)]
  pub fn group(doc: Doc) -> Self {
    Self::Group(Box::new(doc))
  }
}

/// The representation of a document data type.
#[derive(Default, Debug, Eq, PartialEq)]
pub enum SDoc {
  /// An empty document.
  #[default]
  SNil,
  /// A string within a document.
  SText(String, Box<SDoc>),
  /// A new line and spaces.
  SLine(i32, Box<SDoc>),
}
impl SDoc {
  /// Converts a document into a string.
  #[inline]
  fn to_string(sdoc: &SDoc) -> String {
    match sdoc {
      SDoc::SNil => String::with_capacity(0usize),
      SDoc::SText(s, d) => format!("{s}{}", SDoc::to_string(d)),
      SDoc::SLine(indent, d) => {
        let prefix = " ".repeat(*indent as usize);

        format!("\n{prefix}{}", SDoc::to_string(d))
      }
    }
  }
}

/// The representation of a Mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
  /// A flat mode.
  #[default]
  Flat,
  /// A broken  mode.
  Broken,
}

fn fits(w: i32, mut docs: Vec<(i32, Mode, Doc)>) -> bool {
  if w < 0i32 {
    return false;
  }

  while let Some((indent, mode, doc)) = docs.pop() {
    match doc {
      Doc::Nil => continue,
      Doc::Cons(x, y) => {
        docs.push((indent, mode, *y));
        docs.push((indent, mode, *x));
      }
      Doc::Text(s) => {
        let width = w - s.len() as i32;

        return fits(width, docs);
      }
      Doc::Nest(j, x) => {
        docs.push((indent + j, mode, *x));
      }
      Doc::Brk => {
        if mode == Mode::Flat {
          return fits(w - 1, docs);
        } else {
          return true;
        }
      }
      Doc::Group(d) => {
        docs.push((indent, Mode::Flat, *d));
      }
    }
  }

  true
}

/// Formats a document into a formatted one.
pub fn fmt(w: i32, k: i32, mut docs: Vec<(i32, Mode, Doc)>) -> SDoc {
  match docs.pop() {
    None => SDoc::SNil,
    Some((indent, mode, doc)) => match doc {
      Doc::Nil => fmt(w, k, docs),
      Doc::Cons(lhs, rhs) => {
        docs.push((indent, mode, *rhs));
        docs.push((indent, mode, *lhs));
        fmt(w, k, docs)
      }
      Doc::Text(s) => {
        let len = s.len() as i32;

        SDoc::SText(s, Box::new(fmt(w, k + len, docs)))
      }
      Doc::Nest(i, doc) => {
        docs.push((indent + i, mode, *doc));
        fmt(w, k, docs)
      }
      Doc::Brk => {
        if mode == Mode::Flat {
          SDoc::SText(" ".into(), Box::new(fmt(w, k + 1, docs)))
        } else {
          SDoc::SLine(indent, Box::new(fmt(w, indent, docs)))
        }
      }
      Doc::Group(lhs) => {
        if fits(w - k, Vec::from([(indent, Mode::Flat, *lhs.clone())])) {
          docs.push((indent, Mode::Flat, *lhs.clone()));
        } else {
          docs.push((indent, Mode::Broken, *lhs.clone()));
        }

        fmt(w, k, docs)
      }
    },
  }
}

/// Pretty-prints a document into a string.
///
/// #### examples.
///
/// ```
/// 
/// use swisskit::fmt;
/// use swisskit::fmt::Doc;
///
/// let doc = Doc::group(Doc::nest(
///   2,
///   Doc::cons(
///     Doc::group(Doc::text("a".into())),
///     Doc::cons(Doc::text("==".into()), Doc::text("b".into())),
///   ),
/// ));
///
/// println!("{}", fmt::pp(2, doc));
/// ```
pub fn pp(width: i32, doc: Doc) -> String {
  let sdoc = fmt(
    width,
    0,
    Vec::from([(0, Mode::Flat, Doc::Group(Box::new(doc)))]),
  );

  SDoc::to_string(&sdoc)
}
