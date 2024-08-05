# zo.

> *The `zo` programming language.*

## compiler phases.

- `reading` — *read the input and generates the source code as bytes.*
- `tokenizing` — *transforms a sequence of bytes into a list of tokens.*
- `parsing` — *creates an abtract syntax tree from a list of tokens.*
- `analyzing` — *analyses the semantics of a given AST.*
- `codegen` — *generates the code for a specific target.*
- `interpreting` — *interprets a given AST.*
- `building` — *builds the related files based on the target.*

## syntax.

The code below prints the first eleven numbers in the [Fibonacci](https://en.wikipedia.org/wiki/Fibonacci_sequence) sequence:

```rs
-- a simple fibonacci.

fun main() {
  imu fib := fn (n) -> when n < 2 
    ? 1
    : fib(n - 1) + fib(n - 2);

  println("{}", fib(11));
}
```

## goals.

- [ ] fast `compilation-time`.
- [ ] user-friendly `error` messages.
- [ ] metaprogramming.
- [ ] robust `type system`.
- [ ] safe concurrency model — Actor Model like `erlang`.
- [ ] powerfull `tools` — *native REPL, code editor, etc.*

## dirmap.

```
|-- zo                  # The entry point of the compiler.
|-- zo-analyzer         # The semantic analysis phase of the compiler.
|-- zo-ast              # The `zo` abstract syntax tree.
|-- zo-builder          # The builder — used to build the ouput machine code.
|-- zo-codegen          # The code generation phase of the compiler.
|-- zo-codegen-py       # The Python code generation phase of the compiler.
|-- zo-codegen-wasm     # The WASM code generation phase of the compiler.
|-- zo-compiler         # The `zo` compiler.
|-- zo-driver           # The command-line interface of the compiler.
|-- zo-inferencer       # The type system.
|-- zo-interner         # The string interner.
|-- zo-interpreter      # The evaluation phase of the compiler.
|-- zo-interpreter-clif # The Cranelift evaluation phase of the compiler.
|-- zo-interpreter-zo   # The `zo` evaluation phase of the compiler.
|-- zo-notes            # Docs, notes and speeches.
|-- zo-parser           # The syntax analysis of the compiler.
|-- zo-reader           # The reader — used to read the input source code.
|-- zo-reporter         # The reporter — used to generates user-friendly error messages.
|-- zo-samples          # Samples of the `zo` programming language.
|-- zo-session          # The global session of the compiler.
|-- zo-tokenizer        # The lexical analysis of the compiler.
|-- zo-ty               # The types of the `zo` programming language.
|-- zo-value            # Values are used by the `zo` evaluation phase.
```