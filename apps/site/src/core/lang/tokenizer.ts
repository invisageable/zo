import { Kind, type Token } from "./token";

// Pulled from crates/compiler/zo-notes/public/grammar/zo.ebnf. Keep in
// sync if the language adds keywords. When the wasm port lands, this
// list disappears entirely (the real tokenizer is the source of truth).
const KEYWORDS = new Set([
  "abstract", "and", "apply", "as", "await",
  "break", "channel", "continue",
  "else", "enum",
  "false", "ffi", "fn", "for", "fun",
  "group",
  "if", "imu", "is",
  "load", "loop",
  "match", "mut",
  "nursery",
  "pack", "pub",
  "raw", "return",
  "select", "self", "spawn", "state", "struct", "supervise",
  "thread", "true", "type",
  "val",
  "when", "while",
]);

const TYPES = new Set([
  "bool", "bytes", "char", "float", "int", "str", "uint",
]);

// Number-typed suffixes (speculative: not in zo.ebnf yet). When the
// tokenizer sees `_<type>` after a numeric literal, this set decides
// whether to emit a `NumberSuffix` token vs falling through to the
// regular `_<ident>` path.
const NUMBER_SUFFIXES = new Set([
  "s8", "s16", "s32", "s64",
  "u8", "u16", "u32", "u64",
  "f32", "f64",
  "int", "uint", "float",
]);

// Detect & emit `_<type>` as a `NumberSuffix` token when it follows a
// numeric literal. Returns the new `pos` (advanced past the suffix) or
// the original `pos` if no suffix matched.
function consumeNumberSuffix(
  src: string,
  pos: number,
  tokens: Token[],
): number {
  if (src[pos] !== "_") return pos;
  let end = pos + 1;
  while (end < src.length && isIdentCont(src[end])) end++;
  const ident = src.slice(pos + 1, end);
  if (!NUMBER_SUFFIXES.has(ident)) return pos;
  tokens.push({
    kind: Kind.NumberSuffix,
    text: src.slice(pos, end),
    start: pos,
    end,
  });
  return end;
}

// Detect & emit `e[+|-]digits` as a `NumberExponent` token. Returns the
// new `pos` (advanced past the exponent) or the original `pos` when no
// exponent is present.
function consumeNumberExponent(
  src: string,
  pos: number,
  tokens: Token[],
): number {
  if (src[pos] !== "e" && src[pos] !== "E") return pos;
  let end = pos + 1;
  if (src[end] === "+" || src[end] === "-") end++;
  if (!isDigit(src[end])) return pos;
  while (end < src.length && (isDigit(src[end]) || src[end] === "_")) end++;
  tokens.push({
    kind: Kind.NumberExponent,
    text: src.slice(pos, end),
    start: pos,
    end,
  });
  return end;
}

// `<` is intentionally excluded from the lumping walker — otherwise it
// greedily eats into zsx tag starts (e.g., `-</button` would lump as
// `-</` then leave `button>` orphaned). `<` is emitted as its own
// single-char Punctuation token below.
// `;` is also excluded — it's elevated to Keyword (statement terminator)
// and shouldn't lump with adjacent puncts.
const PUNCT_CHARS = new Set("+-*/%=>!&|^~?:,.@#");
const GROUP_CHARS = new Set("{}[]()");

function isWhitespace(ch: string): boolean {
  return ch === " " || ch === "\t" || ch === "\n" || ch === "\r";
}

function isDigit(ch: string): boolean {
  return ch >= "0" && ch <= "9";
}

function isIdentStart(ch: string): boolean {
  return (ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z") || ch === "_";
}

function isIdentCont(ch: string): boolean {
  return isIdentStart(ch) || isDigit(ch);
}

function isAlnum(ch: string): boolean {
  return isDigit(ch)
    || (ch >= "a" && ch <= "z")
    || (ch >= "A" && ch <= "Z");
}

export function tokenize(src: string): Token[] {
  const tokens: Token[] = [];
  let pos = 0;
  // True between a tag-shaped `<` and its matching `>`. Tag brackets and
  // separators (`<`, `>`, `/`) inside this window are NOT emitted as
  // tokens — the highlighter's gap-filling renders them as bare text
  // (white), so structural zsx markers visually fade compared to
  // colorized idents/keywords/events inside the tag.
  let inTag = false;

  while (pos < src.length) {
    const ch = src[pos];

    if (inTag) {
      if (ch === ">") {
        pos++;
        inTag = false;
        continue;
      }
      if (ch === "/") {
        pos++;
        continue;
      }
    }

    if (isWhitespace(ch)) {
      pos++;
      continue;
    }

    // `;` as Keyword — the language's statement terminator reads as
    // structural punctuation but earns the keyword color for emphasis.
    if (ch === ";") {
      tokens.push({ kind: Kind.Keyword, text: ";", start: pos, end: pos + 1 });
      pos++;
      continue;
    }

    // Comments: `-* … *-` block (multi-line), `--` line, `-!` doc.
    if (ch === "-" && src[pos + 1] === "*") {
      const start = pos;
      pos += 2;
      while (pos < src.length && !(src[pos] === "*" && src[pos + 1] === "-")) pos++;
      if (pos < src.length) pos += 2;
      tokens.push({ kind: Kind.Comment, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    if (ch === "-" && (src[pos + 1] === "-" || src[pos + 1] === "!")) {
      const start = pos;
      while (pos < src.length && src[pos] !== "\n") pos++;
      tokens.push({ kind: Kind.Comment, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    // Strings + char literals — same shape, opening quote is the closer.
    if (ch === '"' || ch === "'") {
      const quote = ch;
      const start = pos;
      pos++;
      while (pos < src.length && src[pos] !== quote) {
        if (src[pos] === "\\" && pos + 1 < src.length) pos += 2;
        else pos++;
      }
      if (pos < src.length) pos++; // consume closing quote
      tokens.push({ kind: Kind.String, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    // Numbers with base prefix: 0x.., 0o.., 0b.. — emitted as a 2-char
    // `NumberPrefix` token followed by a `Number` digits token, then an
    // optional `NumberSuffix` (`_u32`, `_i64`, ...).
    if (
      ch === "0"
      && (src[pos + 1] === "x" || src[pos + 1] === "X"
        || src[pos + 1] === "o" || src[pos + 1] === "O"
        || src[pos + 1] === "b" || src[pos + 1] === "B")
    ) {
      tokens.push({ kind: Kind.NumberPrefix, text: src.slice(pos, pos + 2), start: pos, end: pos + 2 });
      pos += 2;
      const digitsStart = pos;
      while (pos < src.length && (isAlnum(src[pos]) || src[pos] === "_")) {
        // Stop when we hit a `_<known-suffix>` so the suffix gets its own
        // colored token. Walk ahead to peek; restore pos if no match.
        if (src[pos] === "_") {
          let lookahead = pos + 1;
          while (lookahead < src.length && isIdentCont(src[lookahead])) lookahead++;
          if (NUMBER_SUFFIXES.has(src.slice(pos + 1, lookahead))) break;
        }
        pos++;
      }
      tokens.push({ kind: Kind.Number, text: src.slice(digitsStart, pos), start: digitsStart, end: pos });
      pos = consumeNumberSuffix(src, pos, tokens);
      continue;
    }

    // Numbers with `b#`, `o#`, `x#` modifier — `b#101`, `o#75`, `x#7f`.
    if (
      (ch === "b" || ch === "o" || ch === "x")
      && src[pos + 1] === "#"
    ) {
      tokens.push({ kind: Kind.NumberPrefix, text: src.slice(pos, pos + 2), start: pos, end: pos + 2 });
      pos += 2;
      const digitsStart = pos;
      while (pos < src.length && (isAlnum(src[pos]) || src[pos] === "_")) {
        if (src[pos] === "_") {
          let lookahead = pos + 1;
          while (lookahead < src.length && isIdentCont(src[lookahead])) lookahead++;
          if (NUMBER_SUFFIXES.has(src.slice(pos + 1, lookahead))) break;
        }
        pos++;
      }
      tokens.push({ kind: Kind.Number, text: src.slice(digitsStart, pos), start: digitsStart, end: pos });
      pos = consumeNumberSuffix(src, pos, tokens);
      continue;
    }

    // Numbers: integer + decimal + underscores (e.g. 1_000_000.5),
    // scientific e-notation (e.g. 1.0e10, 2.5e-3), and optional type
    // suffix (e.g. 42_u32, 3.14_f64). Each component gets its own token
    // so the highlighter can color them distinctly.
    if (isDigit(ch)) {
      const digitsStart = pos;
      while (pos < src.length && (isDigit(src[pos]) || src[pos] === "_" || src[pos] === ".")) {
        if (src[pos] === "_") {
          let lookahead = pos + 1;
          while (lookahead < src.length && isIdentCont(src[lookahead])) lookahead++;
          if (NUMBER_SUFFIXES.has(src.slice(pos + 1, lookahead))) break;
        }
        pos++;
      }
      tokens.push({ kind: Kind.Number, text: src.slice(digitsStart, pos), start: digitsStart, end: pos });
      pos = consumeNumberExponent(src, pos, tokens);
      pos = consumeNumberSuffix(src, pos, tokens);
      continue;
    }

    // Identifiers, keywords, types — disambiguated by lookup.
    if (isIdentStart(ch)) {
      const start = pos;
      while (pos < src.length && isIdentCont(src[pos])) pos++;
      const text = src.slice(start, pos);
      const kind = (text === "true" || text === "false") ? Kind.Boolean
                 : KEYWORDS.has(text) ? Kind.Keyword
                 : TYPES.has(text) ? Kind.Type
                 : Kind.Ident;
      tokens.push({ kind, text, start, end: pos });
      continue;
    }

    if (ch === "<") {
      // HTML-style comment from zsx: `<!-- ... -->`. Multi-line; spans
      // until the closing `-->` (or EOF if unterminated).
      if (src[pos + 1] === "!" && src[pos + 2] === "-" && src[pos + 3] === "-") {
        const start = pos;
        pos += 4;
        while (pos < src.length) {
          if (src[pos] === "-" && src[pos + 1] === "-" && src[pos + 2] === ">") {
            pos += 3;
            break;
          }
          pos++;
        }
        tokens.push({ kind: Kind.Comment, text: src.slice(start, pos), start, end: pos });
        continue;
      }

      // Type-position fragments: only `<>` and `</>` immediately after a
      // standalone `:` (type annotation) read as Type. Everything else
      // — `<button>`, `<a>`, `<` operator — is plain Punctuation, with
      // tag names tokenizing naturally as Ident on the next iteration.
      // The `tokens[last].text === ":"` check is exact: `::`, `:=`,
      // `::=` lump as multi-char tokens via the walker, so they won't
      // match and won't trigger type coloring.
      const inTypePosition = tokens.length > 0
        && tokens[tokens.length - 1].text === ":";
      const next = src[pos + 1];
      const after = src[pos + 2];

      if (inTypePosition) {
        if (next === ">") {
          tokens.push({ kind: Kind.Type, text: "<>", start: pos, end: pos + 2 });
          pos += 2;
          continue;
        }
        if (next === "/" && after === ">") {
          tokens.push({ kind: Kind.Type, text: "</>", start: pos, end: pos + 3 });
          pos += 3;
          continue;
        }
      }

      // Tag-shaped `<`: followed by `/`, `>`, or ident start. Skip
      // emission so the gap renders as white bare text, then enter
      // inTag mode so the matching `>` (and any `/` along the way)
      // also skip emission.
      if (next === ">" || next === "/" || (next !== undefined && isIdentStart(next))) {
        pos++;
        inTag = true;
        continue;
      }

      // Operator `<`: emit as Punctuation, with multi-char forms.
      let opEnd = pos + 1;
      if (next === "=" || next === "<") opEnd++;
      tokens.push({
        kind: Kind.Punctuation,
        text: src.slice(pos, opEnd),
        start: pos,
        end: opEnd,
      });
      pos = opEnd;
      continue;
    }

    // Attributes / directives: `#render`, `#run`, `#asm`, etc. Matched
    // before the generic punctuation walker so `#` + ident lump as one
    // Attribute token instead of splitting into Punctuation + Ident.
    if (ch === "#" && pos + 1 < src.length && isIdentStart(src[pos + 1])) {
      const start = pos;
      pos++;
      while (pos < src.length && isIdentCont(src[pos])) pos++;
      tokens.push({ kind: Kind.Attribute, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    // zsx events: `@click`, `@submit`, etc. Same lump rule as attributes.
    if (ch === "@" && pos + 1 < src.length && isIdentStart(src[pos + 1])) {
      const start = pos;
      pos++;
      while (pos < src.length && isIdentCont(src[pos])) pos++;
      tokens.push({ kind: Kind.Event, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    if (GROUP_CHARS.has(ch)) {
      tokens.push({ kind: Kind.Group, text: ch, start: pos, end: pos + 1 });
      pos++;
      continue;
    }

    if (PUNCT_CHARS.has(ch)) {
      const start = pos;
      while (pos < src.length && PUNCT_CHARS.has(src[pos])) pos++;
      tokens.push({ kind: Kind.Punctuation, text: src.slice(start, pos), start, end: pos });
      continue;
    }

    // Unrecognized: skip a single char to make progress, no token emitted.
    pos++;
  }

  return tokens;
}
