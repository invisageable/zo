import { tokenize } from "./tokenizer";
import { type Span } from "./token";

// Walks the source alongside the token stream so whitespace and any
// unrecognized chars become bare-text spans (kind: null). Renderers
// emit those without a wrapping <span>, preserving spacing in <pre>.
export function highlight(src: string): Span[] {
  const tokens = tokenize(src);
  const spans: Span[] = [];
  let cursor = 0;

  for (const token of tokens) {
    if (token.start > cursor) {
      spans.push({ kind: null, text: src.slice(cursor, token.start) });
    }
    spans.push({ kind: token.kind, text: token.text });
    cursor = token.end;
  }

  if (cursor < src.length) {
    spans.push({ kind: null, text: src.slice(cursor) });
  }

  return spans;
}
