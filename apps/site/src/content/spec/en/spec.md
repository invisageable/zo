# The zo Programming Language Specification.

> *Turn your thoughts into type-safe software and Ui instantly.*

This reference manual is for the zo programming language.

@author — invisageable
@author — compilords

## Disclaimer

> *zo is in early development. We are opening gates, testing some paths. Changes will be made, certain parts work, certain parts don't. Some of them will be removed.*
> 
> *Some features described in this manual may not be available yet. It crafts the foundation as a draft. This document is not the final specification.*
>
> *For any suggestion, keep your focus on reductions to the language. What feature can be combined or omitted? At this point, every ``additive'' feature we're likely to support is already on the table. The task ahead involves combining, trimming, and implementing.*

## Introduction

zo is a child of Rust-prehistory — Graydon Hoare's original Rust — and inherits most of its concepts and philosophy. It is no imitation: zo departs from Rust as it stands today and finds its own niche in zsx (zo Syntax Extension), a builtin for composing cross-platform user interfaces. It is a general-purpose language, suited to building and maintaining applications of any kind.

zo mixes imperative, functional, and concurrent code in a single program. Its concurrency follows communicating sequential processes (CSP): lightweight tasks each keep their own state and coordinate over channels, never through shared memory. Generics and compile-time metaprogramming are built in, and a foreign function interface reaches C directly.

### Goals

zo is built around a few aims:

  - Compile millions of lines a second.
  - Catch whole classes of error before a program runs.
  - Emit diagnostics that people and machines both read.
  - Keep a failure to the task that raised it.
  - Draw one cross-platform interface from a single source.
  - Run many tasks at once.
  - Ship a standard library with the common tools included.
  - Express intent clearly and precisely.
  - Keep the compiler simple.
  - Run programs efficiently.

> *NOTE — the methods behind these goals are proven, not experimental — other languages already ship them. zo's wager is that the right techniques, well combined and made fast, outdo new theory.*

zo is built for the problems of its moment: software that runs unchanged on native targets and the web, that treats the interface as part of the program, and that people and machines read and write together. Those pressures place safety, concurrency, and the developer's experience at the center of the design.

### Features

The sections below highlight zo's most notable features, set against how other languages solve the same problems.

  - **No Null, No Raw Pointers**    

    zo has **no null value**. Where another language returns null to mean "nothing here", zo returns `Option<T>` — `Some(T)` or `None` — so the absence shows up in the type. You cannot reach the value without matching both cases, so a null dereference is not a mistake you can make in zo. The same idea appears as `Maybe` in Elm and `option` in ML.

    zo has **no raw pointers** either: no pointer arithmetic, no address-of-local, nothing to leave dangling.   

  - **Lightweight Tasks and Channels**

    zo runs work as tasks — **lightweight threads** that the language runtime schedules itself. Tasks are cheap to create, and their stacks grow on demand, so each starts small. `spawn f(x)` starts a green task — `spawn thread f(x)` puts the work on its own OS thread. Every `spawn` belongs to a `nursery`, a scope that waits for the tasks inside it to finish before control moves on.

    Tasks usually coordinate through **channels**. `channel()` returns a sender and a receiver — `Tx<T>` and `Rx<T>` — so one task calls `send` while another calls `recv`, and `select` waits on several channels at once. This is the model Go and Newsqueak use. zo's prime sieve is the chain of filter stages Rob Pike first wrote in Newsqueak.

    Shared memory is still allowed where it pays off. A single `Vec` can pass into several tasks that each write a separate region — useful for splitting a grid across workers — though zo leaves it to you to keep those regions disjoint.

  - **zsx — Ui as a builtin**

    Most languages reach for a separate framework to build a user interface. zo builds one in. zsx — the zo Syntax Extension — lets you write interface markup directly in source: elements like `<button>`, bare `<>…</>` fragments, and `{expr}` to splice values in. A `{#html markup}` interpolation splices a string in as raw HTML instead, an opt-out of escaping for markup you trust. Markup has its own type, `</>`, and binds like any other value: `imu view: </> ::= <h2>{title}</h2>;`. The compiler turns it into rendering instructions at build time, so a zo program ships **no UI framework and no bundle**. Writing markup as language literals is an idea zo takes from E4X.

    Markup can do more than show text. `@click={…}` and `@input={…}` attach event handlers. A function that returns `</>` is a component: it takes props as parameters, receives callbacks the same way, and a `<slot/>` marks where a caller's children belong.

    zsx tracks which values a piece of markup reads, so when one changes, only the nodes that used it re-render. Fine-grained reactivity like this comes from Leptos, and SolidJS before it. They reach it with runtime signals. zo reaches it at compile time, with neither signals nor a virtual DOM.

    **One source serves every target**. `#render view;` draws the interface — to the web DOM, to a native window, or to UIKit on iOS, with Android planned. Styles sit beside the markup in a `$: { }` block, scoped to its module unless you mark it `pub`. Inside, CSS properties take short forms — `bg` for background, `c` for color, `ta` for text-align — shorthands zo takes from Imba.

  - **Direct Interface to C**

    A C function enters zo through a `pub ffi` declaration: a signature, no body, resolved at link time. A `#link` block names the dynamic library that owns the symbols. zo then calls the function straight from its signature — arguments go into registers per the platform ABI, scalars narrow or widen, structs pass by value — with **no wrapper layer and no runtime cost**. C strings cross as `CStr`, since a zo `str` carries a length header a C function would misread. zo's raylib and sqlite providers are built on these declarations.

    Writing one `pub ffi` line per function, kept in sync with the library, is the tedious part, so `zo-binder` **writes them for you**. Point it at a Rust crate that exports `extern "C"` functions, or at a C library's machine-readable header, and it emits the `#link` block and one `pub ffi` per function, mapping each C type to its zo equivalent and reporting what it cannot. The result is committed and reviewed like any other source — nothing runs during `zo run`.

  - **Generic Code**

    A function or type can take a **type parameter**, written with a `$` sigil to set it apart from an ordinary name. `fun max<$T>(a: $T, b: $T) -> $T` runs for any `$T`. `struct Vec<$T>` holds elements of one type fixed per use. Functions, structs, enums, and `apply` blocks all take parameters this way. `Vec`, `Option`, `Result`, and `HashMap` are generic types from the standard library, written with the same parameters your own code uses. The `$T` notation comes from Jai.

    zo specializes generics at compile time. Each set of type arguments produces its own code — `Vec<int>` and `Vec<str>` compile to separate, concrete versions, with **no boxing and no runtime type tags**. Because the compiler knows each `$T`'s size and layout where it emits the code, a generic type lays out exactly like its hand-written equivalent. A type parameter can also be bounded by an abstract: a generic written `<$T: Eq>` accepts only types that implement `Eq`, which guarantees the operations its body uses.

  - **Abstracts**

    Generics give one body of code that runs over many types — abstracts give one name to behavior that many types implement differently. The first is parametric polymorphism, the second ad-hoc polymorphism.

    An abstract is a **contract** — a set of method signatures a type can promise to keep. `abstract Display { fun display(self) -> str; }` declares one — the methods take `self`, and `Self` names the implementing type. A type keeps the contract with an `apply` block: `apply Display for Point { fun display(self) -> str { … } }`.

    zo ships `Eq`, `Ord`, and `Show`, wired into the language: `==` and `!=` dispatch to `Eq`'s `eq`, `sort_by` and `min_of` to `Ord`'s `cmp`, and `showln` to `Show`, so any type that applies `Show` can print itself.

    A function reaches an abstract in one of three ways, trading speed for code size. Written `item: Display`, the compiler specializes a copy of the function per concrete type, with no indirection. Written `<$T: Display>(item: $T)`, it specializes the same way but names the parameter, so you can reuse it across the signature or give it more than one bound at once — `<$T: Show + Eq>`. Written `item: any Display`, the value is **boxed behind a vtable**: one copy of the function serves every type, a `[]any Display` can hold a mix of them, and each call pays a runtime lookup.

    Abstracts are typeclasses, in the line of Haskell and Rust's traits.

  - **Local Type Inference**

    zo asks for types at a function's boundary and **infers them inside it**. Every parameter names its type, and a function that returns a value names that too. A local binding takes its type from its initializer instead: `imu n := xs.len()` is an `int` because `len` returns one, and the inferring form `:=` stands in for the explicit `imu n: int = xs.len()`.

    Inference stays local. zo never works backward from a body to guess a parameter or return type — a signature is something you write, and the compiler checks the body against it. Within a body, types flow by unification, the technique **Hindley-Milner** is built on.

  - **Metaprogramming**

    **zo's compiler is an interpreter**: it runs your code at compile time. The pass that checks types is the same one that executes declarations and constant expressions, emitting SIR as it goes — the execution-based model zo takes from Carbon. So metaprogramming in zo happens at build time — there is no runtime reflection and no runtime code generation, because by the time a program runs every structure has already been resolved.

    **Directives** are where you reach this directly. A directive is `#` followed by a name and an argument: `#render view` lowers a template to rendering instructions, and `#link { … }` binds a foreign library. The compiler executes each one while it builds SIR, so a directive generates code without a separate macro or preprocessor pass.

    Derivation works the same way. Mark a struct `%% serialize` and the compiler reads its fields and writes its `to_json` and `from_json`, composing through nested types with no glue by hand. Reading a type's structure to generate code is reflection — in zo it runs in the compiler, not the program.

  - **Errors and Failure**

    zo separates what can go wrong into two kinds. An error you might handle is a value: a function that can fail returns `Result<T, E>`, and one that might come back empty returns `Option<T>`. You read either with `match`, so the failing path shows up in the code and in the type. zo has **no exceptions**.

    A failure you cannot handle is a panic. `panic` ends the task it runs in — not the whole program — and carries a payload that says what happened. zo does not unwind the stack, the task stops where it is, and there is no resuming it.

    A panic does not vanish. A task that awaits a panicked task receives the panic in turn. A `nursery` holds a child's panic at its boundary, a `supervise` scope lets it cascade, cancelling the siblings and travelling past the scope. This is Erlang's "**let it crash**": a failing task dies instead of recovering in place, and its failure is handled at the boundary that contains it.

  - **Static Control Over Mutability**

    A binding is **immutable unless you say otherwise**. `imu x: int = 0` names a value you cannot reassign, `mut x: int = 0` names one you can. The same mark carries into signatures: a parameter or method receiver that a function changes is written `mut`, so the outside sees what a call can alter.

    **Every value has one owner**. Hand an owned value to something that consumes it — a `File`'s `close(own self)`, say — and the old binding is spent, using it again is a compile error.

    When an owner leaves scope, zo frees what it held, so there is **no garbage collector** and, in the common case, nothing to free by hand. The compiler checks that a value is not used after it moves and that it is freed exactly once, it does not borrow-check the way Rust does. This safe-manual-memory model comes from Cyclone and C++'s deterministic destructors.

  - **Helpful Error Messages**

    zo builds each compiler error as an **argument**. Most compilers give you only the conclusion — the bare claim — and leave you to find the reasoning on Stack Overflow. A zo diagnostic carries the whole argument: the claim (the rule you broke), the grounds (the evidence in your code), and, when the fix is mechanical, a resolution. Friendly compiler errors were Elm's idea, from Evan Czaplicki. zo takes the argument structure from Toulmin, after Barik and colleagues showed that the answers developers go looking for are arguments while most compiler errors are not.

    The claim leads. *"expected X, found Y"* is only a ground — a fact with no judgment attached — so zo states the rule first ("type mismatch", "cannot mutate an immutable variable") and shows the conflicting values beneath it. The carets point at those values, never at the operator that joined them: `42 ++ "hello"` underlines `42` and `"hello"`, the two whose types disagree, each labelled with its type. A diagnostic always points at real source, it is never raised without a location.

    A fix and an explanation are kept apart. When the repair is mechanical — `mut` before a binding, a missing `;` — zo attaches it as an edit a tool can apply, not as prose to read. The deeper "why" waits behind `--explain-decisions` until you ask for it.

    **The same diagnostic renders for whoever reads it**. By default it is a colored snippet on stderr, `--format json` and `--format xml` emit the identical content as structured data on stdout, so an editor or an agent acts on the `id`, the span, and the fix without parsing prose.

## Influences

> *« Why accept slow compilers? Just make them faster. » — Jonathan Blow*

> *« Semantic is king. » — Robert Virding*

> *« Re-think traditional compiler design. » — Chandler Carruth*

> *« Performance is king! » — Mike Acton*

> *« millions for compilers, but **hardly a penny for understanding human programming language use.** Now, programming languages are obviously symmetrical, the computor on one side, the human on the other. In an **appropriate science of computer languages,** on would expect that half the effort would be on the computer side, understanding how to translate the languages into executable form, and half on the human side, understanding how to design languages that are **easy or productive to use.** » — John Pane*
---

zo borrows more than it invents. Each of its ideas was proven first in another language — some decades old, some still in active use — and what zo adds is the combination. These are the lineages behind it:

  - Rust (2006), created by Graydon Hoare — the original, pre-1.0 design preserved as the "rust-prehistory" compiler that names this lineage.

  - Carbon (2022), Google's experimental successor to C++, introduced by Chandler Carruth.

  - Jai (2014), Jonathan Blow's systems language for games, long in development. With no published specification, it is known through Blow's two video series — "A Programming Language for Games" and the "Compiler programming livestreams".

  - Erlang (1987), created by Joe Armstrong, Robert Virding, Claes Wikström, Mike Williams, and others at the Ericsson Computer Science Laboratory in Älvsjö, Stockholm.

  - Newsqueak (1988), Rob Pike's concurrent language at Bell Labs — and Go (2009), the systems language designed by Robert Griesemer, Rob Pike, and Ken Thompson at Google.

  - Cyclone (2002), a memory-safe dialect of C, by Trevor Jim, Greg Morrisett, Dan Grossman, Michael Hicks, and others at AT&T Labs Research and Cornell University — and Ada (1983), designed by Jean Ichbiah's team at CII Honeywell Bull for the US Department of Defense, and named after Ada Lovelace.

  - Imba (2015), a web programming language created by Sindre Aarsæther.

  - E4X (ECMA-357, 2004), an extension that put XML literals directly into JavaScript, designed by Terry Lucas and John Schneider — and ECMAScript 4, the language's fourth edition, abandoned in 2008.

  - Elm (2012), Evan Czaplicki's purely functional language for building web interfaces, written as his thesis project.

Other languages lent single features rather than whole designs:

  - The algebraic data types of SML.
  - The macro system of Clojure.
  - The deterministic destructor system of C++.

## Initiation

Just another word for tutorials — See the [initiation](https://zo.compilords.house/initiation)

## Source Code Representation

  - Lexical structure.
  - Grammar.

### Lexical Structure

  - Ignored characters.
  - Identifier tokens.
  - Keyword tokens.
  - Operators and punctuation.
  - Integer literals.
  - Floating-point literals.
  - String and character literals.
  - Bytes literals.
  - Types.
  - zo syntax extension.
  - CSS and shorthands.

#### Ignored Characters

Whitespace and comments lie between tokens and are not tokens themselves. The tokenizer discards both, and zo is not whitespace-sensitive.

Whitespace is any of U+0020 (space), U+0009 (tab, `\t`), U+000A (line feed, `\n`), and U+000D (carriage return, `\r`).

```ebnf
whitespace = ? space | tab | newline | carriage return ? ;
```

A comment is one of three forms, recognized only outside string, character, and template literals:

  - a line comment runs from `--` to the next line feed.
  - a doc comment runs from `-!` to the next line feed, its text meant as documentation.
  - a block comment runs from `-*` to the matching `*-`, and does not nest.

```ebnf
comment       = line_comment | doc_comment | block_comment ;
line_comment  = "--", { ? any character except newline ? }, ? newline ? ;
doc_comment   = "-!", { ? any character except newline ? }, ? newline ? ;
block_comment = "-*", { ? any character sequence not containing "*-" ? }, "*-" ;
```

```zo
-- TODO: add 3 examples here (line, doc and block comments).
```

#### Identifier Tokens

An identifier is a letter or underscore (U+005F) followed by any run of letters, digits (U+0030–U+0039), and underscores. It cannot be spelled like a keyword.

A letter is currently ASCII: U+0041–U+005A and U+0061–U+007A (`A`–`Z`, `a`–`z`). The tokenizer scans identifiers a byte at a time for speed, so it does not yet accept non-ASCII letters — Unicode identifiers, which the grammar below already admits, are a planned extension.

```ebnf
identifier = ( letter | "_" ), { letter | digit | "_" } ;
letter = ? 'a'..'z' | 'A'..'Z' | unicode letter ? ;
digit  = ? '0'..'9' ? ;
```

#### Keyword Tokens

A keyword is a reserved word — it cannot name a binding, function, type, or field. It may still appear as a member name after `.` (`widget.any(pred)` is legal). A few built-in names are not reserved and stay usable as identifiers, notably `channel` (`imu channel: int = 42` is valid).

```ebnf
keyword = "abstract" | "and" | "any" | "apply" | "as" | "await"
        | "break" | "continue" | "else" | "enum" | "false" | "ffi"
        | "fn" | "Fn" | "for" | "fun" | "group" | "if" | "imu" | "is"
        | "load" | "loop" | "match" | "mut" | "nursery" | "own" | "pack"
        | "pub" | "raw" | "return" | "select" | "self" | "Self" | "spawn"
        | "state" | "struct" | "supervise" | "test" | "true" | "type"
        | "val" | "wasm" | "when" | "while" | primitive_type ;
```

#### Operators and Punctuation

Operators bind from tightest to loosest as listed, the `when … ? … :` ternary is looser than all of them.

| Operators | Role |
|---|---|
| `.` `[ ]` `( )` `is` | member access, index, call, type test |
| `!` `-` `+` | not, negate, unary plus |
| `as` | cast |
| `*` `/` `%` | multiply, divide, remainder |
| `+` `++` `-` | add, concatenate, subtract |
| `<<` `>>` | left and right shift |
| `&` `^` `\|` | bitwise and, xor, or |
| `<` `<=` `>` `>=` | ordering |
| `==` `!=` | equality |
| `&&` `..` `..=` | logical and, exclusive and inclusive range |
| `\|\|` | logical or |

The expression grammar encodes this precedence as a chain of rules:

```ebnf
expr = ternary_expr ;
ternary_expr = or_expr, [ "when", or_expr, "?", expr, ":", expr ] ;
or_expr = and_expr, { "||", and_expr } ;
and_expr = range_expr, { "&&", range_expr } ;
range_expr = eq_expr, [ ( ".." | "..=" ), eq_expr ] ;
eq_expr = cmp_expr, { ( "==" | "!=" ), cmp_expr } ;
cmp_expr = bit_or_expr, { ( "<" | ">" | "<=" | ">=" ), bit_or_expr } ;
bit_or_expr = bit_xor_expr, { "|", bit_xor_expr } ;
bit_xor_expr = bit_and_expr, { "^", bit_and_expr } ;
bit_and_expr = shift_expr, { "&", shift_expr } ;
shift_expr = add_expr, { ( "<<" | ">>" ), add_expr } ;
add_expr = mul_expr, { ( "+" | "++" | "-" ), mul_expr } ;
mul_expr = cast_expr, { ( "*" | "/" | "%" ), cast_expr } ;
cast_expr = unary_expr, [ "as", type ] ;
unary_expr = ( "!" | "-" | "+" ), unary_expr | postfix_expr ;
```

Assignment writes back to an existing place, the binding operators introduce a new name instead — `:=` infers the type, `::=` binds a template.

```ebnf
assign_op = "=" | "+=" | "-=" | "*=" | "/=" | "%="
          | "<<=" | ">>=" | "&=" | "|=" | "^=" ;
```

A few operators carry their own meaning: `->` gives a return type, `=>` a match arm or closure body, `::` a path separator, `?` the ternary's second half, and `@` a zsx event attribute.

Delimiters pair an opener with a closer — `( )` parentheses, `{ }` braces, `[ ]` brackets — and `<> … </>` wraps a template fragment.

Punctuation: `,` separates items, `;` ends a statement, `:` ascribes a type, `_` is the wildcard, `%%` introduces an attribute, and `...` is the spread. `#` begins a directive and `$` prefixes a type parameter, raw string, or style block.

#### Integers Literals

An integer literal is a run of digits, with `_` allowed between them as a separator. It may be written in four bases: decimal (`1_000_000`), binary with `0b` (`0b1010`), octal with `0o` (`0o755`), and hexadecimal with `0x` (`0xDE_AD`). A rarer form sets only how the value is displayed: `b#`, `o#`, and `x#` introduce a decimal value with a display base attached — `b#30` is the number 30 shown as binary, `x#76` is 76 shown as hex — so the digits after `#` are always decimal.

```ebnf
int_literal     = decimal_literal | binary_literal | octal_literal
                | hex_literal | base_literal ;
decimal_literal = digit, { digit | "_" } ;
binary_literal  = "0b", binary_digit, { binary_digit | "_" } ;
octal_literal   = "0o", octal_digit, { octal_digit | "_" } ;
hex_literal     = "0x", hex_digit, { hex_digit | "_" } ;
base_literal    = ( "b#" | "o#" | "x#" ), digit, { digit } ;
digit           = ? '0'..'9' ? ;
binary_digit    = ? '0' | '1' ? ;
octal_digit     = ? '0'..'7' ? ;
hex_digit       = ? '0'..'9' | 'a'..'f' | 'A'..'F' ? ;
```

#### Floating-point Literals

A float literal places a decimal point between two runs of digits, carries an exponent, or both. An exponent is `e` or `E`, an optional sign, and decimal digits: `3.14`, `6.022e23`, `1.0e-9`.

```ebnf
float_literal = decimal_literal, ".", decimal_literal, [ exponent ]
              | decimal_literal, exponent ;
exponent      = ( "e" | "E" ), [ "+" | "-" ], decimal_literal ;
```

#### String and Character Literals

A string literal is double-quoted UTF-8 text, and a string is immutable. A character literal is a single Unicode character between single quotes — `'a'`, `'🙂'` — of type `char`. Both accept escape sequences:

  - `\n` `\r` `\t` `\\` `\"` `\'` `\0` — newline, carriage return, tab, backslash, the two quotes, and null.
  - `\a` `\b` `\e` `\f` `\v` — bell, backspace, escape, form feed, and vertical tab.
  - `\xHH` — a byte from two hex digits, as in `\x48`.
  - `\u{…}` — a Unicode scalar from a hex code point, as in `\u{1F680}`.

Inside a string, `{expr}` splices a value in — `"hello, {name}"` — and a literal brace is written `\{` or `\}`.

```ebnf
string_literal = '"', { string_char | "{", expr, "}" }, '"' ;
string_char = char_escape | ? any unicode character except '"', '\', '{', '}' and newline ? ;
char_literal = "'", ( char_escape | ? any unicode character except '\' and newline ? ), "'" ;
char_escape = "\", ( "n" | "r" | "t" | "\" | "'" | '"' | "0"
            | "a" | "b" | "e" | "f" | "v" | "{" | "}"
            | "x", hex_digit, hex_digit
            | "u", "{", hex_digit, { hex_digit }, "}" ) ;
```

A raw string is prefixed with `$` and processes no escapes, so every character stands for itself — useful for paths and patterns: `$"C:\path\to\file"`.

```ebnf
raw_string_literal = "$", '"', { ? any character except '"' ? }, '"' ;
```

#### Bytes Literals

A bytes literal is delimited by backticks and evaluates to a `bytes` value — a byte buffer, not a UTF-8 string. It accepts the same escape sequences as a string literal but does not interpolate, so `\x48\x65` is two bytes.

```ebnf
bytes_literal = "`", { bytes_char }, "`" ;
bytes_char = char_escape | ? any unicode character except '`', '\' and newline ? ;
```

#### Types

...

##### Boolean Types

The boolean type is `bool`, whose only values are the literals `true` and `false`.

```ebnf
boolean_type = "bool" ;
bool_literal = "true" | "false" ;
```

##### Numeric Types.

The default integer is `int`, a signed 32-bit value, `uint` is its unsigned counterpart. The fixed-width integers are `s8`, `s16`, `s32`, `s64` and `u8`, `u16`, `u32`, `u64`. The default floating-point type is `float`, a 64-bit double, beside the fixed widths `f32` and `f64`.

```ebnf
numeric_type = integer_type | float_type ;
integer_type = "int" | "uint" | "s8" | "s16" | "s32" | "s64"
             | "u8" | "u16" | "u32" | "u64" ;
float_type = "float" | "f32" | "f64" ;
```

##### String Types

`str` is an immutable UTF-8 string, `char` is a single Unicode scalar, and `bytes` is a raw byte buffer.

```ebnf
text_type = "str" | "char" | "bytes" ;
```

##### Array Types

An array type wraps an element type in brackets: `[N]T` is a fixed array of `N` elements, and `[]T` is a dynamically sized one.

```ebnf
array_type = "[", [ int_literal ], "]", type ;
```

##### Function Types

A function type is written `Fn(…) -> T`: the parameter types in parentheses, then the return type after the arrow.

```ebnf
function_type = "Fn", "(", [ type, { ",", type } ], ")", "->", type ;
```

##### Abstract Types

...

##### Enumeration Types

...

##### Struct Types

...

##### Block Expressions

...

#### zo Syntax Extension

...

#### CSS and Shorthands

...

### Grammar

The complete grammar is available as an [EBNF file](https://github.com/invisageable/zo/blob/main/crates/compiler/zo-notes/public/grammar/zo.ebnf).

## References

### Language Architecture and Philosophy

  - **ECMAScript for XML (E4X) Specification**

    ECMA-357 Standard, 2nd Edition. ECMA International (2005).    
    [www-archive.mozilla.org/js/language/ECMA-357.pdf](https://www-archive.mozilla.org/js/language/ECMA-357.pdf)    
    *Primary reference for implementing native declarative markup literals.*    

  - **A Programming Language for Games**

    Blow, J. (2014).    
    [youtube.com/watch?v=TH9VCN6UkyQ&list=PLmV5I2fxaiCKfxMBrNsU1kgKJXD3PkyxO](https://www.youtube.com/watch?v=TH9VCN6UkyQ&list=PLmV5I2fxaiCKfxMBrNsU1kgKJXD3PkyxO)    
    *Foundational lecture series and wiki on the Jai compiler architecture, introducing the `$` generic sigil and compile-time execution models.*   

  - **Modernizing Compiler Design for the Carbon Toolchain**

    Carruth, C. (2023).   
    [youtube.com/watch?v=ZI198eFghJk](https://www.youtube.com/watch?v=ZI198eFghJk)    
    *Reference for execution-based semantic analysis and unified compile-time interpreter design.*    

  - **CSS Snapshot**

    W3C Recommendation. World Wide Web Consortium.    
    [w3.org/TR/css](https://www.w3.org/TR/css/)    
    *Specification for syntax-extension style sheets (`$: {}`).*    

  - **The Go Programming Language**

    Pike, R., Thompson, K., Griesemer, R. (2009). Google.   
    [go.dev](https://go.dev)    
    *Reference for lightweight thread schedulers and CSP channel communication.*    

  - **Communicating Sequential Processes (CSP)**    

    Hoare, C. A. R. (1978). *Communicating Sequential Processes*. Communications of the ACM, 21(8), 666-677.    
    [cs.cmu.edu/~crary/819-f09/Hoare78.pdf](https://www.cs.cmu.edu/~crary/819-f09/Hoare78.pdf)    
    *This paper established the mathematical foundation for zo's lightweight concurrent tasks, channels, and select-waiting model.*   

  - **Syntax Across Languages**

    Rigaux, P.    
    [rigaux.org/language-study/syntax-across-languages](https://rigaux.org/language-study/syntax-across-languages)    
    *Comparative survey on programming language syntax boundaries.*   

  - **Concrete syntax matters, actually**

    Lim, S. (2026).   
    [youtube.com/watch?v=kQjrcSMYpaA](https://www.youtube.com/watch?v=kQjrcSMYpaA)    
    *Analysis of syntax-driven developer ergonomics and compiler parsing bounds.*   

  - **Whither Web Programming**

    Bracha, G. (2014).    
    [infoq.com/presentations/web-programming-future](https://www.infoq.com/presentations/web-programming-future)    
    *Conceptual model for the future of in-browser/client-side execution.*    

### Front-End: Lexing, Parsing and Formatting

  - **Beating the Fastest Lexer Generator in Rust**

    Alic, A. (2023).    
    [alic.dev/blog/fast-lexing](https://alic.dev/blog/fast-lexing)    
    *Core technique used to implement zo's high-speed, byte-by-byte lexical analyzer.*    

  - **Some Strategies For Fast Lexical Analysis when Parsing Programming Languages**

    Barrett, S. (2015).   
    [nothings.org/computer/lexing.html](https://nothings.org/computer/lexing.html)    
    *Optimization guidelines for low-overhead token streaming.*   

  - **A Prettier Printer**

    Wadler, P. (1995).    
    [homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf](https://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf)    
    *The algebraic foundation for zo's upcoming layout-solving formatter (zo-fmt).*   

  - **Strictly Pretty**

    Lindig, C. (2000).    
    [researchgate.net/publication/2629249_Strictly_Pretty](www.researchgate.net/publication/2629249_Strictly_Pretty)    
    *Strict-evaluation optimizations for Wadler-style pretty printers.*   

  - **A New Design for Pretty Printer Implementations in Rust**

    Zhuang, J. (2026).    
    [blog.wybxc.cc/blog/pretty-printer-pye](https://blog.wybxc.cc/blog/pretty-printer-pye)    
    **Modern Rust layout-implementation techniques used to structure the formatter’s engine.**    

### Semantics, Types and Memory model

  - **A Modular Module System**

    Leroy, X. (1996). Journal of Functional Programming, 6(3), 269–310.   
    [xavierleroy.org/publi/modular-modules-jfp.pdf](https://xavierleroy.org/publi/modular-modules-jfp.pdf)    
    *Theoretical framework for zo's modular, encapsulated namespace and import-seed architecture.*    

  - **Safe Manual Memory Management in Cyclone**

    Swamy, N., Hicks, M., Morrisett, G., Grossman, D., and Jim, T. (2006). Science of Computer Programming, 62(2), 122-144.   
    [cs.umd.edu/projects/PL/cyclone/scp.pdf](https://www.cs.umd.edu/projects/PL/cyclone/scp.pdf)    
    *The core reference for zo's affine/unique pointer move invariants and lexical scope-exit drops.*   

  - **Generalizing Hindley-Milner Type Inference Algorithms**

    Heeren, B., and Hage, J. (2002).    
    [researchgate.net/publication/2528716_Generalizing_Hindley-Milner_Type_Inference_Algorithms](http://www.researchgate.net/publication/2528716_Generalizing_Hindley-Milner_Type_Inference_Algorithms)   
    *Framework for zo's unification-based local type-inference solver.*   

  - **Typestate-Oriented Programming**

    Aldrich, J., Sunshine, J., Saini, D., and Sparks, Z. (2009). In Onward! (pp. 1015-1022).    
    [cs.cmu.edu/~aldrich/papers/onward2009-state.pdf](https://www.cs.cmu.edu/~aldrich/papers/onward2009-state.pdf)    
    *Reference for future state-transition checking on top of affine lifetimes.*    

  - **A lazy, concurrent convertibility checker**

    Courant, N., and Leroy, X. (2026).    
    [xavierleroy.org/publi/concurrent-convertibility.pdf](https://xavierleroy.org/publi/concurrent-convertibility.pdf)    
    *State-of-the-art concurrent type checking algorithms.*   

  - **Efficient Extensional Binary Tries**

    Appel, A. W., and Leroy, X. (2023). Journal of Functional Programming, 33, e1.    
    [xavierleroy.org/publi/extensional-binary-tries.pdf](xavierleroy.org/publi/extensional-binary-tries.pdf)    
    *Reference for optimal, canonical data-structures in compiler symbol tables.*   

  - **Mechanizing Proofs about Mendler-style Recursion**

    Jacob-Rao, R., Cave, A., and Pientka, B. (2016).    
    [cs.mcgill.ca/~bpientka/papers/lfmtp16.pdf](https://www.cs.mcgill.ca/~bpientka/papers/lfmtp16.pdf)    
    *Algebraic formulation of structural recursion.*    

### Optimizations

  - **Value Numbering**

    Briggs, P., Cooper, K. D., and Simpson, L. T. (1994). Rice University.    
    [softlib.rice.edu/pub/CRPC-TRs/reports/CRPC-TR94517-S.pdf](https://softlib.rice.edu/pub/CRPC-TRs/reports/CRPC-TR94517-S.pdf)    
    *Foundational paper for Global Value Numbering (GVN) optimizations.*    

  - **Combining Analyses, Combining Optimizations**

    Click, C. N. (1995). ACM.   
    [dl.acm.org/doi/epdf/10.1145/201059.201061](https://dl.acm.org/doi/epdf/10.1145/201059.201061)    
    *The formal basis for combining dead-code elimination with constant folding inside your compiler.*    

  - **Extensible Records with Scoped Labels**

    Leijen, D. (2005). Microsoft Research.    
    [microsoft.com/en-us/research/wp-content/uploads/2016/02/scopedlabels.pdf](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/scopedlabels.pdf)    
    *Reference for flexible, type-safe struct extensions.*    

  - **Design and Implementation of an Optimizing Type-Centric Compiler**

    Petrashko, D. (2017). EPFL.   
    [infoscience.epfl.ch/server/api/core/bitstreams/f98a9701-2e68-477e-b9c2-2550c29cb867/content](https://infoscience.epfl.ch/server/api/core/bitstreams/f98a9701-2e68-477e-b9c2-2550c29cb867/content)    
    *The implementation basis for Dotty's optimizing type-directed transformations.*    

  - **Identifying Compiler Optimizations that Break Constant-Time Programming Techniques**

    Willi, F. (2025). ETH Zürich.   
    [ethz.ch/content/dam/ethz/special-interest/infk/inst-infsec/appliedcrypto/education/theses/semester-project_fiona-willi.pdf](https://ethz.ch/content/dam/ethz/special-interest/infk/inst-infsec/appliedcrypto/education/theses/semester-project_fiona-willi.pdf)    
    *Critical reference for ensuring compiler optimizations do not introduce side-channel vulnerabilities.*   

  - **Investigating Magic Numbers: Improving the Inlining Heuristic in the Glasgow Haskell Compiler**

    Hollenbeck, C., O'Boyle, M. F. P., and Steuwer, M. (2022). In Proceedings of the Haskell Symposium (pp. 57-69).   
    [dl.acm.org/doi/10.1145/3546189.3549918](https://dl.acm.org/doi/10.1145/3546189.3549918)    
    *Optimization heuristics for inlining decisions.*   

### Code Generation, Linking and Targets

  - **OS X ABI Mach-O File Format Reference**

    Apple Developer.    
    [github.com/aidansteele/osx-abi-macho-file-format-reference](https://github.com/aidansteele/osx-abi-macho-file-format-reference)    
    *Specification of Mach-O headers, segments, and load commands used to build zo's in-process linker.*    

  - **The Intel 64 and IA-32 Architectures Software Developer’s Manual**

    Intel Developer (2006).   
    [homes.di.unimi.it/sisop/lucidi0607/253669.pdf](https://homes.di.unimi.it/sisop/lucidi0607/253669.pdf)    
    *Specification of the x86_64 instruction and calling convention.*   

  - **Arm® Architecture Reference Manual**

    ARM Developer.    
    [developer.arm.com/documentation/ddi0487/mb](https://developer.arm.com/documentation/ddi0487/mb)    
    *Specification for ARMv8-A (AArch64) encoding used by zo's native code generator and register-swapping assembly.*   

  - **Program Fragments, Linking, and Modularization**

    Cardelli, L. (1997).    
    [lucacardelli.name/Papers/Linking.A4.pdf](http://lucacardelli.name/Papers/Linking.A4.pdf)   
    *The theoretical model governing how independent compiled objects link securely.*   

### Diagnostics, Documentation and Human Factors

  - **How Should Compilers Explain Problems to Developers?**

    Barik, T., Ford, D., Murphy-Hill, E., and Parnin, C. (2018). In Proceedings of the ACM Joint Meeting on European Software Engineering Conference and Symposium on the Foundations of Software Engineering (pp. 633-645).    
    [static.barik.net/barik/publications/fse2018/barik_fse18.pdf](https://static.barik.net/barik/publications/fse2018/barik_fse18.pdf)    
    *The direct cognitive and empirical basis for zo's structured claims-and-grounds diagnostic layouts.*   

  - **Type-Error Ablation and AI Coding Agents**

    Krishnamurthi, S., and Flatt, M. (2026).    
    [arxiv.org/pdf/2606.01522](arxiv.org/pdf/2606.01522)    
    *State-of-the-art research on compiler diagnostic architectures designed to be consumed by LLM agents.*   

  - **What I think about when I edit**

    Parish, E. (2019).    
    [eva-parish.squarespace.com/blog/how-i-edit](eva-parish.squarespace.com/blog/how-i-edit)    
    *Stylistic editing principles used to structure the language's reference manual.*   

### Performance and Energy Dynamics

  - **Energy Efficiency Across Programming Languages**

    Pereira, R., Couto, M., Ribeiro, F., Rua, R., Cunha, J., Fernandes, J. P., and Saraiva, J. (2017). In Proceedings of the International Conference on Software Language Engineering (pp. 256-267).   
    [greenlab.di.uminho.pt/wp-content/uploads/2017/10/sleFinal.pdf](greenlab.di.uminho.pt/wp-content/uploads/2017/10/sleFinal.pdf)    
    *The empirical study demonstrating the strict relationship between compiler optimization, GC-avoidance, and hardware energy efficiency.*    

### Inspirational Repositories

Projects than I found interesting in some aspect.

#### Compilers

  - **Rust Prehistory**

    Graydon Hoare, Andreas Gal, Brendan Eich, Dave Herman, Michael Bebenita, Patrick Walton and Roy Frostig (2006)    
    [github.com/graydon/rust-prehistory](https://github.com/graydon/rust-prehistory)    
    *...*   

  - **L4C**

    Alex Crichton   
    [github.com/alexcrichton/l4c](https://github.com/alexcrichton/l4c)    
    *...*   

  - **Yak**

    Greg Melton's toy programming language inspired by bits of Rust, Go, Python, and JavaScript.    
    [github.com/grippy/yak](https://github.com/grippy/yak)    
    *...*   
 
  - **ES4**  
  
    Graydon Hoare, Jeff Dyer, Lars T Hansen,  Dave Herman, Brendan Eich, Brian Crowder and Blake Kaplan original, collaborative Adobe/Mozilla ECMAScript 4.   
    [github.com/bkero/es4](https://github.com/bkero/es4)    
    *...*   
 
  - **rue** 

    Steve Klabnik   
    [github.com/rue-language/rue](https://github.com/rue-language/rue)    
    *...*   
 
  - **Imba**

    Sindre Aarsaether   
    [github.com/imba/imba](https://github.com/imba/imba)    
    *...*   

#### Runtime

  - **Lunatic**

    Bernard Kolobara's WebAssembly actor runtime    
    [github.com/lunatic-solutions/lunatic](https://github.com/lunatic-solutions/lunatic)    
    *...*   

  - **JQuery**

    OpenJS Foundation (the original, lightweight DOM querying standard).    
    [api.jquery.com](https://api.jquery.com)    
    *...*   

  - **goquery**

    Martin Angers' Go-based document-traversal library.   
    [github.com/PuerkitoBio/goquery](https://github.com/PuerkitoBio/goquery)    
    *...*   

#### Type System

  - **Typical**

    Ravern Koh.   
    [github.com/ravern/typical](https://github.com/ravern/typical)    
    *...*   

#### Benchmark

  - **The Computer Language Benchmarks Game**

    Fabian Beuke    
    [github.com/madnight/benchmarksgame](https://github.com/madnight/benchmarksgame)    
    *...*   
