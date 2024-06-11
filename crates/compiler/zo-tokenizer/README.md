# zo-tokenizer.

> *The `lexical analysis` phase.*

## about.

The `tokenizer` module which produces a stream of tokens that represent the lexical structure of the input.    

## dirmap.

```
src                       # ...
|-- token                 # the token kind splits by file.
    |-- comment.rs        # the representation of a `comment` token.
    |-- group.rs          # the representation of a `group` token.
    |-- int.rs            # some instance related to a `integer` token.
    |-- kw.rs             # the representation of a `keyword` token.
    |-- op.rs             # the representation of a `operator` token.
    |-- punctuation.rs    # the representation of a `punctation` token.
|-- lib.rs                # import all the files related by the tokenizer.
|-- state.rs              # a state used by the tokenizer. it contains each step needed to scan a `zo` file.
|-- token.rs              # the representation of a `comment` token.
|-- tokenizer.rs          # the tokenizer instance.
```
