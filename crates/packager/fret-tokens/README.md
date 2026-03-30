# fret — tokens.

> *The token definition for the fret.*

## about.

TOKEN TYPES FOR THE `fret.oz` CONFiGURATiON FORMAT.

## types.

  - `TokenKind` — *enum of all token variants (`String`, `Number`, `Ident`, etc.).*
  - `Token` — *kind + byte range (`start`, `end`) into source text.*

## dev.

ZERO-ALLOCATiON DESiGN — TOKENS STORE BYTE OFFSETS iNTO THE SOURCE TEXT, NOT OWNED STRiNGS.
