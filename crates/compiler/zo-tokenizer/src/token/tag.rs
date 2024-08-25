use german_str::GermanStr;
use hashbrown::HashMap;
use thin_vec::ThinVec;

type HtmlTagNames = HashMap<GermanStr, Html>;
type AtomTagNames = HashMap<GermanStr, Atom>;
type MathMlTagNames = HashMap<GermanStr, MathMl>;
type SvgTagNames = HashMap<GermanStr, Svg>;
type CustomTagNames = HashMap<GermanStr, Custom>;

/// The representation of zo syntax extension (zsx).
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  /// A tag kind — see also [`TagKind`].
  pub kind: TagKind,
  /// A tag name — see also [`Name`].
  // pub name: String,
  pub name: Name,
  /// A self closing tag flag.
  pub self_closing: bool,
  /// A fragment tag flag.
  pub frag: bool,
  /// A list of attributes — see also [`Attr`].
  pub attrs: ThinVec<Attr>,
}

impl Tag {
  /// Creates a new tag.
  #[inline(always)]
  pub fn new(
    kind: TagKind,
    name: Name,
    self_closing: bool,
    frag: bool,
    attrs: ThinVec<Attr>,
  ) -> Self {
    Self {
      kind,
      name,
      self_closing,
      frag,
      attrs,
    }
  }
}

impl std::fmt::Display for Tag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      kind,
      name,
      self_closing,
      frag,
      attrs,
    } = self;

    match kind {
      TagKind::Opening => {
        if *frag {
          write!(f, "<>")
        } else if *self_closing {
          write!(f, "<{name} {attrs:?} />")
        } else {
          write!(f, "<{name} {attrs:?}>")
        }
      }
      TagKind::Closing => {
        if *frag {
          write!(f, "</>")
        } else {
          write!(f, "</{name}>")
        }
      }
    }
  }
}

/// The representation of zo syntax extension (zsx).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TagKind {
  /// An opening tag.
  Opening,
  /// A closing tag.
  Closing,
}

impl std::fmt::Display for TagKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Opening => write!(f, "<tag-opening>"),
      Self::Closing => write!(f, "</tag-closing>"),
    }
  }
}

/// The representation of a tag name.
///
/// A name must follow the kebab-case naming convention.
#[derive(Clone, Debug, PartialEq)]
pub enum Name {
  /// An atom name.
  Atom(Atom),
  /// A html name.
  Html(Html),
  /// A mathml name.
  MathMl(MathMl),
  /// A svg name.
  Svg(Svg),
  /// A custom name.
  Custom(Custom),
}

impl Name {
  /// Gets the right name kind from an name value.
  #[inline]
  pub fn from_name(name: &str) -> Self {
    let atom = ATOM_TAG_NAMES.get(name);
    let html = HTML_TAG_NAMES.get(name);
    let math = MATHML_TAG_NAMES.get(name);
    let svg = SVG_TAG_NAMES.get(name);

    match name {
      _ if atom.is_some() => Self::Atom(*atom.unwrap()),
      _ if html.is_some() => Self::Html(*html.unwrap()),
      _ if math.is_some() => Self::MathMl(*math.unwrap()),
      _ if svg.is_some() => Self::Svg(*svg.unwrap()),
      "_" => Self::Custom(Custom::Fragment),
      _ => Self::Custom(Custom::Name(name.into())),
    }
  }
}

impl std::fmt::Display for Name {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Atom(name) => write!(f, "{name}"),
      Self::Html(name) => write!(f, "{name}"),
      Self::MathMl(name) => write!(f, "{name}"),
      Self::Svg(name) => write!(f, "{name}"),
      Self::Custom(name) => write!(f, "{name}"),
    }
  }
}

/// The representation of an attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
  /// A static attribute — `foo="bar"`.
  Static(String, Option<String>),
  /// A dynamic attribute — `foo={bar}`, `{bar}`.
  Dynamic(String, Option<String>),
}

impl std::fmt::Display for Attr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(name, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{name}={value}")
        } else {
          write!(f, "{name}")
        }
      }
      Self::Dynamic(name, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{name}={{{value}}}")
        } else {
          write!(f, "{{{name}}}")
        }
      }
    }
  }
}

/// The representation of a key tag name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Atom {
  /// An `:bind` atom.
  Bind,
  /// An `:else` if atom.
  Else,
  /// A `:for` atom.
  For,
  /// An `:if` atom.
  If,
  /// A `:?` atom.
  Question,
  /// An `:while` atom.
  While,
  /// A wildcard atom — `:`.
  Wildcard,
}

lazy_static::lazy_static! {
  /// A static map of custom tag name elements.
  pub static ref ATOM_TAG_NAMES: AtomTagNames = HashMap::from([
    (GermanStr::new_inline(":bin"), Atom::Bind),
    (GermanStr::new_inline(":else"), Atom::Else),
    (GermanStr::new_inline(":for"), Atom::For),
    (GermanStr::new_inline(":if"), Atom::If),
    (GermanStr::new_inline(":?"), Atom::Question),
    (GermanStr::new_inline(":while"), Atom::While),
    (GermanStr::new_inline(":"), Atom::Wildcard),
  ]);
}

impl std::fmt::Display for Atom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Bind => write!(f, ":bin"),
      Self::Else => write!(f, ":else"),
      Self::For => write!(f, ":for"),
      Self::If => write!(f, ":if"),
      Self::Question => write!(f, ":?"),
      Self::While => write!(f, ":while"),
      Self::Wildcard => write!(f, ":"),
    }
  }
}

/// The representation of html tag name.
///
/// see — https://www.w3.org/TR/2012/WD-html-markup-20121025/elements.html.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Html {
  /// The a element represents a hyperlink.
  A,
  /// The abbr element represents an abbreviation or acronym.
  Abbr,
  /// The address element represents contact information.
  Address,
  /// The area element represents either a hyperlink with some text and a
  /// corresponding area on an image map, or a dead area on an image map.
  Area,
  /// The article element represents a section of content that forms an
  /// independent part of a document or site; for example, a magazine or
  /// newspaper article, or a blog entry.
  Article,
  /// The aside element represents content that is tangentially related to the
  /// content that forms the main textual flow of a document.
  Aside,
  /// An audio element represents an audio stream.
  Audio,
  /// The b element represents a span of text offset from its surrounding
  /// content without conveying any extra emphasis or importance, and for which
  /// the conventional typographic presentation is bold text; for example,
  /// keywords in a document abstract, or product names in a review.
  B,
  /// The base element specifies a document-wide base URL for the purposes of
  /// resolving relative URLs, and a document-wide default browsing context
  /// name for the purposes of following hyperlinks.
  Base,
  /// The bdi element represents a span of text that is isolated from its
  /// surroundings for the purposes of bidirectional text formatting [BIDI].
  Bdi,
  /// The bdo element represents an explicit text directionality formatting
  /// control for its children; it provides a means to specify a direction
  /// override of the Unicode BiDi algorithm [BIDI].
  Bdo,
  /// The blockquote element represents a section that is quoted from another
  /// source.
  BlockQuote,
  /// The body element represents the body of a document (as opposed to the
  /// document’s metadata).
  Body,
  /// The br element represents a line break.
  Br,
  /// The button element with a type attribute whose value is "submit"
  /// represents a button for submitting a form.
  Button,
  /// The canvas element represents a resolution-dependent bitmap canvas, which
  /// can be used for dynamically rendering of images such as game graphics,
  /// graphs, or other images.
  Canvas,
  /// The caption element represents the title of the table that is its parent.
  Caption,
  /// The cite element represents the cited title of a work; for example, the
  /// title of a book mentioned within the main text flow of a document.
  Cite,
  /// The code element represents a fragment of computer code.
  Code,
  /// The col element represents one or more columns in the column group
  /// represented by its colgroup parent.
  Col,
  /// The colgroup element represents a group of one or more columns in the
  /// table that is its parent.
  Colgroup,
  /// The command element is a multipurpose element for representing commands.
  Command,
  /// The datalist element represents a set of option elements that represent
  /// predefined options for other controls.
  Datalist,
  /// The dd element represents a description or value.
  Dd,
  /// The del element represents a range of text that has been deleted from a
  /// document.
  Del,
  /// The details element represents a control from which the user can obtain
  /// additional information or controls on-demand.
  Details,
  /// The dfn element represents the defining instance of a term.
  Dfn,
  /// The div element is a generic container for flow content that by itself
  /// does not represent anything.
  Div,
  /// The dl element represents a description list.
  Dl,
  /// The dt element represents a term or name.
  Dt,
  /// The em element represents a span of text with emphatic stress.
  Em,
  /// The embed element represents an integration point for external content.
  Embed,
  /// The fieldset element represents a set of form controls grouped under a
  /// common name.
  Fieldset,
  /// The figcaption element represents a caption or legend for a figure.
  Figcaption,
  /// The figure element represents a unit of content, optionally with a
  /// caption, that is self-contained, that is typically referenced as a single
  /// unit from the main flow of the document, and that can be moved away from
  /// the main flow of the document without affecting the document’s meaning.
  Figure,
  /// The footer element represents the footer for the section it applies to.
  Footer,
  /// The form element represents a user-submittable form.
  Form,
  /// The h1 through h6 elements are headings for the sections with which they
  /// are associated.
  H1,
  /// The h1 through h6 elements are headings for the sections with which they
  ///  are associated.
  H2,
  /// The h1 through h6 elements are headings for the sections with which they
  ///  are associated.
  H3,
  /// The h1 through h6 elements are headings for the sections with which they
  ///  are associated.
  H4,
  /// The h1 through h6 elements are headings for the sections with which they
  ///  are associated.
  H5,
  /// The h1 through h6 elements are headings for the sections with which they
  /// are associated.
  H6,
  /// The head element collects the document’s metadata.
  Head,
  /// The header element represents the header of a section.
  Header,
  /// The hgroup element represents a group of headings.
  Hgroup,
  /// The hr element represents a paragraph-level thematic break.
  Hr,
  /// The html element represents the root of a document.
  Html,
  /// The i element represents a span of text offset from its surrounding
  /// content without conveying any extra emphasis or importance, and for which
  /// the conventional typographic presentation is italic text; for example, a
  /// taxonomic designation, a technical term, an idiomatic phrase from another
  /// language, a thought, or a ship name.
  I,
  /// The iframe element introduces a new nested browsing context.
  Iframe,
  /// The img element represents an image.
  Img,
  /// The input element is a multipurpose element for representing input
  /// controls.
  Input,
  /// The ins element represents a range of text that has been inserted (added)
  /// to a document.
  Ins,
  /// The kbd element represents user input.
  Kbd,
  /// The keygen element represents a control for generating a public-private
  /// key pair and for submitting the public key from that key pair.
  Keygen,
  /// The label element represents a caption for a form control.
  Label,
  /// The legend element represents a title or explanatory caption for the rest
  /// of the contents of the legend element’s parent element.
  Legend,
  /// The li element represents a list item.
  Li,
  /// The link element represents metadata that expresses inter-document
  /// relationships.
  Link,
  /// The map element, in conjunction with any area element descendants,
  /// defines an image map.
  Map,
  /// The mark element represents a run of text in one document marked or
  /// highlighted for reference purposes, due to its relevance in another
  /// context.
  Mark,
  /// The menu element represents a list of commands.
  Menu,
  /// The meta element is a multipurpose element for representing metadata.
  Meta,
  /// The meter element represents a scalar gauge providing a measurement
  /// within a known range, or a fractional value.
  Meter,
  /// The nav element represents a section of a document that links to other
  /// documents or to parts within the document itself; that is, a section of
  /// navigation links.
  Nav,
  /// The noscript element is used to present different markup to user agents
  /// that don’t support scripting, by affecting how the document is parsed.
  Noscript,
  /// The object element represents external content.
  Object,
  /// The ol element represents a list (or sequence) of items; that is, a list
  /// in which the items are intentionally ordered, such that changing the
  /// order would change the meaning of the list.
  Ol,
  /// The optgroup element represents a group of option elements with a common
  /// label.
  Optgroup,
  /// The option element represents an option in a select control, or an option
  /// in a labelled set of options grouped together in an optgroup, or an
  /// option among the list of suggestions in a datalist.
  Option,
  /// The output element represents the result of a calculation.
  Output,
  /// The p element represents a paragraph.
  P,
  /// The param element defines parameters for plugins invoked by object
  /// elements.
  Param,
  /// The pre element represents a block of preformatted text, in which
  /// structure is represented by typographic conventions rather than by
  /// elements.
  Pre,
  /// The progress element represents the completion progress of a task.
  Progress,
  /// The q element represents phrasing content quoted from another source.
  Q,
  /// The rp element can be used to provide parentheses around a ruby text
  /// component of a ruby annotation, to be shown by UAs that don’t support
  /// ruby annotations.
  Rp,
  /// The rt element marks the ruby text component of a ruby annotation.
  Rt,
  /// The ruby element allows spans of phrasing content to be marked with ruby
  /// annotations.
  Ruby,
  /// The s element represents contents that are no longer accurate or no
  /// longer relevant and that therefore has been “struck” from the document.
  S,
  /// The samp element represents (sample) output from a program or computing
  /// system.
  Samp,
  /// The script element enables dynamic script and data blocks to be included
  /// in documents.
  Script,
  /// The section element represents a section of a document, typically with a
  /// title or heading.
  Section,
  /// The select element represents a control for selecting among a list of
  /// options.
  Select,
  /// The small element represents so-called “fine print” or “small print”,
  /// such as legal disclaimers and caveats.
  Small,
  /// The source element enables multiple media sources to be specified for
  /// audio and video elements.
  Source,
  /// The span element is a generic wrapper for phrasing content that by itself
  /// does not represent anything.
  Span,
  /// The strong element represents a span of text with strong importance.
  Strong,
  /// The style element allows style information to be embedded in documents.
  Style,
  /// The sub element represents subscript.
  Sub,
  /// The summary element represents a summary, caption, or legend for a
  /// details element.
  Summary,
  /// The sup element represents superscript.
  Sup,
  /// The table element represents a table; that is, data with more than one
  /// dimension.
  Table,
  /// The tbody element represents a block of rows that consist of a body of
  /// data for its parent table element.
  Tbody,
  /// The td element represents a data cell in a table.
  Td,
  /// The textarea element represents a multi-line plain-text edit control for
  /// the element’s raw value.
  Textarea,
  /// The tfoot element represents the block of rows that consist of the column
  /// summaries (footers) for its parent table element.
  Tfoot,
  /// The th element represents a header cell in a table.
  Th,
  /// The thead element represents the block of rows that consist of the column
  /// labels (headings) for its parent table element.
  Thead,
  /// The time element represents a date and/or time.
  Time,
  /// The title element represents the document’s title or name.
  Title,
  /// The tr element represents a row of cells in a table.
  Tr,
  /// The track element enables supplementary media tracks such as subtitle
  /// tracks and caption tracks to be specified for audio and video elements.
  Track,
  /// The u element represents a span of text offset from its surrounding
  /// content without conveying any extra emphasis or importance, and for which
  /// the conventional typographic presentation is underlining; for example, a
  /// span of text in Chinese that is a proper name (a Chinese proper name
  /// mark), or span of text that is known to be misspelled.
  U,
  // The ul element represents an unordered list of items; that is, a list in
  // which changing the order of the items would not change the meaning of
  // list.
  Ul,
  /// The var element represents either a variable in a mathematical expression
  /// or programming context, or placeholder text that the reader is meant to
  /// mentally replace with some other literal value.
  Var,
  /// The video element represents a video or movie.
  Video,
  /// The wbr element represents a line-break opportunity.
  Wbr,
}

impl Html {
  /// Checks if an element is an empty one (void element).
  #[inline(always)]
  pub fn is_void_elmt(&self) -> bool {
    matches!(
      self,
      Self::Area
        | Self::Base
        | Self::Br
        | Self::Col
        | Self::Command
        | Self::Embed
        | Self::Hr
        | Self::Img
        | Self::Input
        | Self::Keygen
        | Self::Link
        | Self::Meta
        | Self::Param
        | Self::Source
        | Self::Track
        | Self::Wbr
    )
  }

  /// Checks if an element is a raw text.
  #[inline(always)]
  pub fn is_raw_text_elmt(&self) -> bool {
    matches!(self, Self::Script | Self::Style)
  }

  /// Checks if an element is an escapable raw text.
  #[inline(always)]
  pub fn is_escapable_raw_text_elmt(&self) -> bool {
    matches!(self, Self::Textarea | Self::Title)
  }
}

impl std::fmt::Display for Html {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::A => write!(f, "a"),
      Self::Abbr => write!(f, "abbr"),
      Self::Address => write!(f, "address"),
      Self::Area => write!(f, "area"),
      Self::Article => write!(f, "article"),
      Self::Aside => write!(f, "aside"),
      Self::Audio => write!(f, "audio"),
      Self::B => write!(f, "b"),
      Self::Base => write!(f, "base"),
      Self::Bdi => write!(f, "bdi"),
      Self::Bdo => write!(f, "bdo"),
      Self::BlockQuote => write!(f, "blockquote"),
      Self::Body => write!(f, "body"),
      Self::Br => write!(f, "br"),
      Self::Button => write!(f, "button"),
      Self::Canvas => write!(f, "canvas"),
      Self::Caption => write!(f, "caption"),
      Self::Cite => write!(f, "cite"),
      Self::Code => write!(f, "code"),
      Self::Col => write!(f, "col"),
      Self::Colgroup => write!(f, "colgroup"),
      Self::Command => write!(f, "command"),
      Self::Datalist => write!(f, "datalist"),
      Self::Dd => write!(f, "dd"),
      Self::Del => write!(f, "del"),
      Self::Details => write!(f, "details"),
      Self::Dfn => write!(f, "dfn"),
      Self::Div => write!(f, "div"),
      Self::Dl => write!(f, "dl"),
      Self::Dt => write!(f, "dt"),
      Self::Em => write!(f, "em"),
      Self::Embed => write!(f, "embed"),
      Self::Fieldset => write!(f, "fieldset"),
      Self::Figcaption => write!(f, "figcaption"),
      Self::Figure => write!(f, "figure"),
      Self::Footer => write!(f, "footer"),
      Self::Form => write!(f, "form"),
      Self::H1 => write!(f, "h1"),
      Self::H2 => write!(f, "h2"),
      Self::H3 => write!(f, "h3"),
      Self::H4 => write!(f, "h4"),
      Self::H5 => write!(f, "h5"),
      Self::H6 => write!(f, "h6"),
      Self::Head => write!(f, "head"),
      Self::Header => write!(f, "header"),
      Self::Hgroup => write!(f, "hgroup"),
      Self::Hr => write!(f, "hr"),
      Self::Html => write!(f, "html"),
      Self::I => write!(f, "i"),
      Self::Iframe => write!(f, "iframe"),
      Self::Img => write!(f, "img"),
      Self::Input => write!(f, "input"),
      Self::Ins => write!(f, "ins"),
      Self::Kbd => write!(f, "kdb"),
      Self::Keygen => write!(f, "keygen"),
      Self::Label => write!(f, "label"),
      Self::Legend => write!(f, "legend"),
      Self::Li => write!(f, "li"),
      Self::Link => write!(f, "link"),
      Self::Map => write!(f, "map"),
      Self::Mark => write!(f, "mark"),
      Self::Menu => write!(f, "menu"),
      Self::Meta => write!(f, "meta"),
      Self::Meter => write!(f, "meter"),
      Self::Nav => write!(f, "nav"),
      Self::Noscript => write!(f, "noscript"),
      Self::Object => write!(f, "object"),
      Self::Ol => write!(f, "ol"),
      Self::Optgroup => write!(f, "optgroup"),
      Self::Option => write!(f, "option"),
      Self::Output => write!(f, "output"),
      Self::P => write!(f, "p"),
      Self::Param => write!(f, "param"),
      Self::Pre => write!(f, "pre"),
      Self::Progress => write!(f, "progress"),
      Self::Q => write!(f, "q"),
      Self::Rp => write!(f, "rp"),
      Self::Rt => write!(f, "rt"),
      Self::Ruby => write!(f, "ruby"),
      Self::S => write!(f, "s"),
      Self::Samp => write!(f, "samp"),
      Self::Script => write!(f, "script"),
      Self::Section => write!(f, "section"),
      Self::Select => write!(f, "select"),
      Self::Small => write!(f, "small"),
      Self::Source => write!(f, "source"),
      Self::Span => write!(f, "span"),
      Self::Strong => write!(f, "strong"),
      Self::Style => write!(f, "style"),
      Self::Sub => write!(f, "sub"),
      Self::Summary => write!(f, "summary"),
      Self::Sup => write!(f, "sup"),
      Self::Table => write!(f, "table"),
      Self::Tbody => write!(f, "tbody"),
      Self::Td => write!(f, "td"),
      Self::Textarea => write!(f, "textarea"),
      Self::Tfoot => write!(f, "tfoot"),
      Self::Th => write!(f, "th"),
      Self::Thead => write!(f, "thead"),
      Self::Time => write!(f, "time"),
      Self::Title => write!(f, "title"),
      Self::Tr => write!(f, "tr"),
      Self::Track => write!(f, "track"),
      Self::U => write!(f, "u"),
      Self::Ul => write!(f, "ul"),
      Self::Var => write!(f, "var"),
      Self::Video => write!(f, "video"),
      Self::Wbr => write!(f, "wbr"),
    }
  }
}

lazy_static::lazy_static! {
  /// A static map of html tag name elements.
  pub static ref HTML_TAG_NAMES: HtmlTagNames = HashMap::from([
    (GermanStr::new_inline("a"), Html::A),
    (GermanStr::new_inline("abbr"), Html::Abbr),
    (GermanStr::new_inline("address"), Html::Address),
    (GermanStr::new_inline("area"), Html::Area),
    (GermanStr::new_inline("article"), Html::Article),
    (GermanStr::new_inline("aside"), Html::Aside),
    (GermanStr::new_inline("audio"), Html::Audio),
    (GermanStr::new_inline("b"), Html::B),
    (GermanStr::new_inline("base"), Html::Base),
    (GermanStr::new_inline("bdi"), Html::Bdi),
    (GermanStr::new_inline("bdo"), Html::Bdo),
    (GermanStr::new_inline("blockquote"), Html::BlockQuote),
    (GermanStr::new_inline("body"), Html::Body),
    (GermanStr::new_inline("br"), Html::Br),
    (GermanStr::new_inline("button"), Html::Button),
    (GermanStr::new_inline("canvas"), Html::Canvas),
    (GermanStr::new_inline("caption"), Html::Caption),
    (GermanStr::new_inline("cite"), Html::Cite),
    (GermanStr::new_inline("code"), Html::Code),
    (GermanStr::new_inline("col"), Html::Col),
    (GermanStr::new_inline("colgroup"), Html::Colgroup),
    (GermanStr::new_inline("command"), Html::Command),
    (GermanStr::new_inline("datalist"), Html::Datalist),
    (GermanStr::new_inline("dd"), Html::Dd),
    (GermanStr::new_inline("del"), Html::Del),
    (GermanStr::new_inline("details"), Html::Details),
    (GermanStr::new_inline("dfn"), Html::Dfn),
    (GermanStr::new_inline("div"), Html::Div),
    (GermanStr::new_inline("dl"), Html::Dl),
    (GermanStr::new_inline("dt"), Html::Dt),
    (GermanStr::new_inline("em"), Html::Em),
    (GermanStr::new_inline("embed"), Html::Embed),
    (GermanStr::new_inline("fieldset"), Html::Fieldset),
    (GermanStr::new_inline("figcaption"), Html::Figcaption),
    (GermanStr::new_inline("figure"), Html::Figure),
    (GermanStr::new_inline("footer"), Html::Footer),
    (GermanStr::new_inline("form"), Html::Form),
    (GermanStr::new_inline("h1"), Html::H1),
    (GermanStr::new_inline("h2"), Html::H2),
    (GermanStr::new_inline("h3"), Html::H3),
    (GermanStr::new_inline("h4"), Html::H4),
    (GermanStr::new_inline("h5"), Html::H5),
    (GermanStr::new_inline("h6"), Html::H6),
    (GermanStr::new_inline("head"), Html::Head),
    (GermanStr::new_inline("header"), Html::Header),
    (GermanStr::new_inline("hgroup"), Html::Hgroup),
    (GermanStr::new_inline("hr"), Html::Hr),
    (GermanStr::new_inline("html"), Html::Html),
    (GermanStr::new_inline("i"), Html::I),
    (GermanStr::new_inline("iframe"), Html::Iframe),
    (GermanStr::new_inline("img"), Html::Img),
    (GermanStr::new_inline("input"), Html::Input),
    (GermanStr::new_inline("ins"), Html::Ins),
    (GermanStr::new_inline("kdb"), Html::Kbd),
    (GermanStr::new_inline("keygen"), Html::Keygen),
    (GermanStr::new_inline("label"), Html::Label),
    (GermanStr::new_inline("legend"), Html::Legend),
    (GermanStr::new_inline("li"), Html::Li),
    (GermanStr::new_inline("link"), Html::Link),
    (GermanStr::new_inline("map"), Html::Map),
    (GermanStr::new_inline("mark"), Html::Mark),
    (GermanStr::new_inline("menu"), Html::Menu),
    (GermanStr::new_inline("meta"), Html::Meta),
    (GermanStr::new_inline("meter"), Html::Meter),
    (GermanStr::new_inline("nav"), Html::Nav),
    (GermanStr::new_inline("noscript"), Html::Noscript),
    (GermanStr::new_inline("object"), Html::Object),
    (GermanStr::new_inline("ol"), Html::Ol),
    (GermanStr::new_inline("optgroup"), Html::Optgroup),
    (GermanStr::new_inline("option"), Html::Option),
    (GermanStr::new_inline("output"), Html::Output),
    (GermanStr::new_inline("p"), Html::P),
    (GermanStr::new_inline("param"), Html::Param),
    (GermanStr::new_inline("pre"), Html::Pre),
    (GermanStr::new_inline("progress"), Html::Progress),
    (GermanStr::new_inline("q"), Html::Q),
    (GermanStr::new_inline("rp"), Html::Rp),
    (GermanStr::new_inline("rt"), Html::Rt),
    (GermanStr::new_inline("ruby"), Html::Ruby),
    (GermanStr::new_inline("s"), Html::S),
    (GermanStr::new_inline("samp"), Html::Samp),
    (GermanStr::new_inline("script"), Html::Script),
    (GermanStr::new_inline("section"), Html::Section),
    (GermanStr::new_inline("select"), Html::Select),
    (GermanStr::new_inline("small"), Html::Small),
    (GermanStr::new_inline("source"), Html::Source),
    (GermanStr::new_inline("span"), Html::Span),
    (GermanStr::new_inline("strong"), Html::Strong),
    (GermanStr::new_inline("style"), Html::Style),
    (GermanStr::new_inline("sub"), Html::Sub),
    (GermanStr::new_inline("summary"), Html::Summary),
    (GermanStr::new_inline("sup"), Html::Sup),
    (GermanStr::new_inline("table"), Html::Table),
    (GermanStr::new_inline("tbody"), Html::Tbody),
    (GermanStr::new_inline("td"), Html::Td),
    (GermanStr::new_inline("textarea"), Html::Textarea),
    (GermanStr::new_inline("tfoot"), Html::Tfoot),
    (GermanStr::new_inline("th"), Html::Th),
    (GermanStr::new_inline("thead"), Html::Thead),
    (GermanStr::new_inline("time"), Html::Time),
    (GermanStr::new_inline("title"), Html::Title),
    (GermanStr::new_inline("tr"), Html::Tr),
    (GermanStr::new_inline("track"), Html::Track),
    (GermanStr::new_inline("u"), Html::U),
    (GermanStr::new_inline("ul"), Html::Ul),
    (GermanStr::new_inline("var"), Html::Var),
    (GermanStr::new_inline("video"), Html::Video),
    (GermanStr::new_inline("wbr"), Html::Wbr),
  ]);
}

/// The representation of mathml foreign tag.
/// see — https://developer.mozilla.org/en-US/docs/Web/MathML/Element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MathMl {
  /// A nath tag name — top-level element.
  Math,
}

lazy_static::lazy_static! {
  /// A static map of mathml foreign tag name elements.
  pub static ref MATHML_TAG_NAMES: MathMlTagNames = HashMap::from([
    (GermanStr::new_inline("math"), MathMl::Math),
  ]);
}

impl std::fmt::Display for MathMl {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Math => write!(f, "svg"),
    }
  }
}

/// The representation of svg foreign tag.
/// see — https://developer.mozilla.org/en-US/docs/Web/SVG/Element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Svg {
  /// An anchor tag name.
  A,
  /// A svg tag name.
  Svg,
}

lazy_static::lazy_static! {
  /// A static map of svg foreign tag name elements.
  pub static ref SVG_TAG_NAMES: SvgTagNames = HashMap::from([
    (GermanStr::new_inline("a"), Svg::A),
    (GermanStr::new_inline("svg"), Svg::Svg),
  ]);
}

impl std::fmt::Display for Svg {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::A => write!(f, "a"),
      Self::Svg => write!(f, "svg"),
    }
  }
}

/// The representation of a custom tag name.
#[derive(Clone, Debug, PartialEq)]
pub enum Custom {
  /// A fragment tag name.
  Fragment,
  /// A custom tag name.
  Name(String),
}

lazy_static::lazy_static! {
  /// A static map of custom tag name elements.
  pub static ref CUSTOM_TAG_NAMES: CustomTagNames = HashMap::from([
    (GermanStr::new_inline("_"), Custom::Fragment),
  ]);
}

impl std::fmt::Display for Custom {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Fragment => write!(f, "_"),
      Self::Name(name) => write!(f, "{name}"),
    }
  }
}

static NAMESPACES: &[(&str, &str)] = &[
  ("", ""),
  ("*", "*"),
  ("html", "http://www.w3.org/1999/xhtml"),
  ("xml", "http://www.w3.org/XML/1998/namespace"),
  ("xmlns", "http://www.w3.org/2000/xmlns/"),
  ("xlink", "http://www.w3.org/1999/xlink"),
  ("svg", "http://www.w3.org/2000/svg"),
  ("mathml", "http://www.w3.org/1998/Math/MathML"),
];
