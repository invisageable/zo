# fret — tokens.

## about.

token types for the fret.oz configuration format. zero-allocation design — tokens store byte offsets into the source text, not owned strings.

## types.

- `TokenKind` — enum of all token variants (`String`, `Number`, `Identifier`, `Pack`, `@`, `(`, `)`, etc.)
- `Token` — kind + byte range (`start`, `end`) into source text
