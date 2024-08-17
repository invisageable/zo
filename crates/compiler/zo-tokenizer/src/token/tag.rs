use smol_str::SmolStr;
use zo_interner::interner::symbol::Symbol;

use swisskit::fmt::sep_space;

use hashbrown::HashMap;
use thin_vec::ThinVec;

type HtmlTagNames = HashMap<SmolStr, Html>;

/// The representation of zo syntax extension (zsx).
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  /// A tag kind — see also [`TagKind`].
  pub kind: TagKind,
  /// A name — see also [`Name`].
  pub name: Name,
  /// A self closing tag flag.
  pub self_closing: bool,
  /// A list of attributes — see also [`Attr`].
  pub attrs: ThinVec<Attr>,
}

impl std::fmt::Display for Tag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Self {
      kind,
      name,
      self_closing,
      attrs,
    } = self;

    match kind {
      TagKind::Opening => {
        if *self_closing {
          write!(f, "<{name} {attrs} />", attrs = sep_space(attrs))
        } else {
          write!(f, "<{name} {attrs}>", attrs = sep_space(attrs))
        }
      }
      TagKind::Closing => write!(f, "</tag-closing>"),
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

/// The representation of an name.
///
/// A name must follow the kebab-case naming convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Name {
  /// A html name.
  Html(Html),
  /// A custom name.
  Custom(Symbol),
}

impl std::fmt::Display for Name {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Html(html) => write!(f, "{html}"),
      Self::Custom(sym) => write!(f, "{sym}"),
    }
  }
}

/// The representation of an attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attr {
  /// A static attribute — `foo="bar"`.
  Static(Symbol, Option<Symbol>),
  /// A dynamic attribute — `foo={bar}`, `{bar}`.
  Dynamic(Symbol, Option<Symbol>),
}

impl std::fmt::Display for Attr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(sym, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{sym}=\"{value}\"")
        } else {
          write!(f, "{sym}")
        }
      }
      Self::Dynamic(sym, maybe_value) => {
        if let Some(value) = maybe_value {
          write!(f, "{sym}={{{value}}}")
        } else {
          write!(f, "{{{sym}}}")
        }
      }
    }
  }
}

/// The representation of html tag name.
///
/// see — https://www.w3.org/TR/2012/WD-html-markup-20121025/elements.html.
/// An anchor tag name — `<a>`.
/// An div tag name — `<div>`.
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
  #[inline]
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
  // A static map of html elements.
  pub static ref HTML_TAG_NAMES: HtmlTagNames = HashMap::from([
    (SmolStr::new_inline("a"), Html::A),
    (SmolStr::new_inline("abbr"), Html::Abbr),
    (SmolStr::new_inline("address"), Html::Address),
    (SmolStr::new_inline("area"), Html::Area),
    (SmolStr::new_inline("article"), Html::Article),
    (SmolStr::new_inline("aside"), Html::Aside),
    (SmolStr::new_inline("audio"), Html::Audio),
    (SmolStr::new_inline("b"), Html::B),
    (SmolStr::new_inline("base"), Html::Base),
    (SmolStr::new_inline("bdi"), Html::Bdi),
    (SmolStr::new_inline("bdo"), Html::Bdo),
    (SmolStr::new_inline("blockquote"), Html::BlockQuote),
    (SmolStr::new_inline("body"), Html::Body),
    (SmolStr::new_inline("br"), Html::Br),
    (SmolStr::new_inline("button"), Html::Button),
    (SmolStr::new_inline("canvas"), Html::Canvas),
    (SmolStr::new_inline("caption"), Html::Caption),
    (SmolStr::new_inline("cite"), Html::Cite),
    (SmolStr::new_inline("code"), Html::Code),
    (SmolStr::new_inline("col"), Html::Col),
    (SmolStr::new_inline("colgroup"), Html::Colgroup),
    (SmolStr::new_inline("command"), Html::Command),
    (SmolStr::new_inline("datalist"), Html::Datalist),
    (SmolStr::new_inline("dd"), Html::Dd),
    (SmolStr::new_inline("del"), Html::Del),
    (SmolStr::new_inline("details"), Html::Details),
    (SmolStr::new_inline("dfn"), Html::Dfn),
    (SmolStr::new_inline("div"), Html::Div),
    (SmolStr::new_inline("dl"), Html::Dl),
    (SmolStr::new_inline("dt"), Html::Dt),
    (SmolStr::new_inline("em"), Html::Em),
    (SmolStr::new_inline("embed"), Html::Embed),
    (SmolStr::new_inline("fieldset"), Html::Fieldset),
    (SmolStr::new_inline("figcaption"), Html::Figcaption),
    (SmolStr::new_inline("figure"), Html::Figure),
    (SmolStr::new_inline("footer"), Html::Footer),
    (SmolStr::new_inline("form"), Html::Form),
    (SmolStr::new_inline("h1"), Html::H1),
    (SmolStr::new_inline("h2"), Html::H2),
    (SmolStr::new_inline("h3"), Html::H3),
    (SmolStr::new_inline("h4"), Html::H4),
    (SmolStr::new_inline("h5"), Html::H5),
    (SmolStr::new_inline("h6"), Html::H6),
    (SmolStr::new_inline("head"), Html::Head),
    (SmolStr::new_inline("header"), Html::Header),
    (SmolStr::new_inline("hgroup"), Html::Hgroup),
    (SmolStr::new_inline("hr"), Html::Hr),
    (SmolStr::new_inline("html"), Html::Html),
    (SmolStr::new_inline("i"), Html::I),
    (SmolStr::new_inline("iframe"), Html::Iframe),
    (SmolStr::new_inline("img"), Html::Img),
    (SmolStr::new_inline("input"), Html::Input),
    (SmolStr::new_inline("ins"), Html::Ins),
    (SmolStr::new_inline("kdb"), Html::Kbd),
    (SmolStr::new_inline("keygen"), Html::Keygen),
    (SmolStr::new_inline("label"), Html::Label),
    (SmolStr::new_inline("legend"), Html::Legend),
    (SmolStr::new_inline("li"), Html::Li),
    (SmolStr::new_inline("link"), Html::Link),
    (SmolStr::new_inline("map"), Html::Map),
    (SmolStr::new_inline("mark"), Html::Mark),
    (SmolStr::new_inline("menu"), Html::Menu),
    (SmolStr::new_inline("meta"), Html::Meta),
    (SmolStr::new_inline("meter"), Html::Meter),
    (SmolStr::new_inline("nav"), Html::Nav),
    (SmolStr::new_inline("noscript"), Html::Noscript),
    (SmolStr::new_inline("object"), Html::Object),
    (SmolStr::new_inline("ol"), Html::Ol),
    (SmolStr::new_inline("optgroup"), Html::Optgroup),
    (SmolStr::new_inline("option"), Html::Option),
    (SmolStr::new_inline("output"), Html::Output),
    (SmolStr::new_inline("p"), Html::P),
    (SmolStr::new_inline("param"), Html::Param),
    (SmolStr::new_inline("pre"), Html::Pre),
    (SmolStr::new_inline("progress"), Html::Progress),
    (SmolStr::new_inline("q"), Html::Q),
    (SmolStr::new_inline("rp"), Html::Rp),
    (SmolStr::new_inline("rt"), Html::Rt),
    (SmolStr::new_inline("ruby"), Html::Ruby),
    (SmolStr::new_inline("s"), Html::S),
    (SmolStr::new_inline("samp"), Html::Samp),
    (SmolStr::new_inline("script"), Html::Script),
    (SmolStr::new_inline("section"), Html::Section),
    (SmolStr::new_inline("select"), Html::Select),
    (SmolStr::new_inline("small"), Html::Small),
    (SmolStr::new_inline("source"), Html::Source),
    (SmolStr::new_inline("span"), Html::Span),
    (SmolStr::new_inline("strong"), Html::Strong),
    (SmolStr::new_inline("style"), Html::Style),
    (SmolStr::new_inline("sub"), Html::Sub),
    (SmolStr::new_inline("summary"), Html::Summary),
    (SmolStr::new_inline("sup"), Html::Sup),
    (SmolStr::new_inline("table"), Html::Table),
    (SmolStr::new_inline("tbody"), Html::Tbody),
    (SmolStr::new_inline("td"), Html::Td),
    (SmolStr::new_inline("textarea"), Html::Textarea),
    (SmolStr::new_inline("tfoot"), Html::Tfoot),
    (SmolStr::new_inline("th"), Html::Th),
    (SmolStr::new_inline("thead"), Html::Thead),
    (SmolStr::new_inline("time"), Html::Time),
    (SmolStr::new_inline("title"), Html::Title),
    (SmolStr::new_inline("tr"), Html::Tr),
    (SmolStr::new_inline("track"), Html::Track),
    (SmolStr::new_inline("u"), Html::U),
    (SmolStr::new_inline("ul"), Html::Ul),
    (SmolStr::new_inline("var"), Html::Var),
    (SmolStr::new_inline("video"), Html::Video),
    (SmolStr::new_inline("wbr"), Html::Wbr),
  ]);
}
