# fret — tokenizer.

## about.

zero-allocation tokenizer for the fret.oz configuration format. operates directly on byte slices and produces tokens on-demand via `next_token()`.

## features.

- on-demand tokenization (no pre-scan)
- escape sequence handling in strings
- line comments (`--`)
- identifier/keyword discrimination (`pack`)
