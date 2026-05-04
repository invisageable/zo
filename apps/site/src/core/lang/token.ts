// Kind values are the CSS class names rendered by the highlighter (e.g.,
// `<span class="keyword">fun</span>`), keeping the styling layer untouched
// if the tokenizer is ever swapped for a wasm port. Single source of
// truth shared by tokenizer + highlighter + CSS.
export enum Kind {
  Keyword = "keyword",
  Ident = "ident",
  Group = "group",
  Punctuation = "punctuation",
  Number = "number",
  Comment = "comment",
  String = "string",
  Type = "type",
  Attribute = "attribute",
  Event = "event",
  Boolean = "boolean",
}

export interface Token {
  kind: Kind;
  text: string;
  start: number;
  end: number;
}

// A render-ready slice. `kind` is null for plain whitespace/text gaps
// between tokens — the highlighter emits those as bare text so spacing
// is preserved without wrapping in a span.
export interface Span {
  kind: Kind | null;
  text: string;
}