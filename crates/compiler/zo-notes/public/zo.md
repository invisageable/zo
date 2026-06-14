# zo.

> *Turn your thoughts into type-safe software and Ui instantly.*

This reference manual is for the zo programming language.

@author — invisageable
@author — compilords

> *DiSCLAiMER — zo is in early development. We are opening gates, testing some paths. Changes will be made, certain parts work, certain parts don't. Some of them will be removed.*
> 
> *Some features described in this manual may not be available yet. It crafts the foundation as a draft. This document is not the final specification.*
>
> *For any suggestion, keep your focus on reductions to the language. What feature can be combined or omitted? At this point, every ``additive'' feature we're likely to support is already on the table. The task ahead involves combining, trimming, and implementing.*

## introduction.

zo is a child of Rust-prehistory — Graydon Hoare's original Rust — and inherits most of its concepts and philosophy. It is no imitation: zo departs from Rust as it stands today and finds its own niche in zsx (zo Syntax Extension), a builtin for composing cross-platform user interfaces. It is a general-purpose language, suited to building and maintaining applications of any kind.

zo supports a mixture of imperative, functional, and concurrent styles. Its concurrency model isolates lightweight tasks that share no mutable state and communicate by passing messages over channels, in the manner of communicating sequential processes (CSP). It also supports generic programming and compile-time metaprogramming, and calls C directly through a foreign function interface.

## goals.

The language design pursues the following goals:

  - Compilation at millions of lines per second.
  - Compile-time elimination of whole classes of error.
  - Diagnostics legible to humans and machines alike.
  - Run-time fault tolerance and containment.
  - Cross-platform user interfaces from a single source.
  - High concurrency.
  - A batteries-included standard library.
  - Clarity and precision of expression.
  - Implementation simplicity.
  - Run-time efficiency.

> *NOTE — the methods behind these goals are proven, not experimental — other languages already ship them. zo's wager is that the right techniques, well combined and made fast, outdo new theory.*

zo is built for the problems of its moment: software that runs unchanged on native targets and the web, that treats the interface as part of the program, and that people and machines read and write together. Those pressures place safety, concurrency, and the developer's experience at the center of the design.

## features.

The sections below highlight zo's most notable features, set against how other languages solve the same problems.

### no null, no raw pointers.

zo has no null value. Where another language returns null to mean "nothing here", zo returns `Option<T>` — `Some(T)` or `None` — so the absence shows up in the type. You cannot reach the value without matching both cases, so a null dereference is not a mistake you can make in zo. The same idea appears as `Maybe` in Elm and `option` in ML.

zo has no raw pointers either: no pointer arithmetic, no address-of-local, nothing to leave dangling.

### lightweight tasks and channels.

zo runs work as tasks — lightweight threads that the language runtime schedules itself. Tasks are cheap to create, and their stacks grow on demand, so each starts small. `spawn f(x)` starts a green task; `spawn thread f(x)` puts the work on its own OS thread. Every `spawn` belongs to a `nursery`, a scope that waits for the tasks inside it to finish before control moves on.

Tasks usually coordinate through channels. `channel()` returns a sender and a receiver — `Tx<T>` and `Rx<T>` — so one task calls `send` while another calls `recv`, and `select` waits on several channels at once. This is the model Go and Newsqueak use; zo's prime sieve is the chain of filter stages Rob Pike first wrote in Newsqueak.

Shared memory is still allowed where it pays off. A single `Vec` can pass into several tasks that each write a separate region — useful for splitting a grid across workers — though zo leaves it to you to keep those regions disjoint.

### zsx — ui as a builtin.

Most languages reach for a separate framework to build a user interface. zo builds one in. zsx — the zo Syntax Extension — lets you write interface markup directly in source: elements like `<button>`, bare `<>…</>` fragments, and `{expr}` to splice values in. A `{#html markup}` interpolation splices a string in as raw HTML instead, an opt-out of escaping for markup you trust. Markup has its own type, `</>`, and binds like any other value: `imu view: </> ::= <h2>{title}</h2>;`. The compiler turns it into rendering instructions at build time, so a zo program ships no UI framework and no bundle. Writing markup as language literals is an idea zo takes from E4X.

Markup can do more than show text. `@click={…}` and `@input={…}` attach event handlers. A function that returns `</>` is a component: it takes props as parameters, receives callbacks the same way, and a `<slot/>` marks where a caller's children belong.

zsx tracks which values a piece of markup reads, so when one changes, only the nodes that used it re-render. Fine-grained reactivity like this comes from Leptos, and SolidJS before it. They reach it with runtime signals; zo reaches it at compile time, with neither signals nor a virtual DOM.

One source serves every target. `#render view;` draws the interface — to the web DOM, to a native window, or to UIKit on iOS, with Android planned. Styles sit beside the markup in a `$: { }` block, scoped to its module unless you mark it `pub`. Inside, CSS properties take short forms — `bg` for background, `c` for color, `ta` for text-align — shorthands zo takes from Imba.

### direct interface to C.

A C function enters zo through a `pub ffi` declaration: a signature, no body, resolved at link time. A `#link` block names the dynamic library that owns the symbols. zo then calls the function straight from its signature — arguments go into registers per the platform ABI, scalars narrow or widen, structs pass by value — with no wrapper layer and no runtime cost. C strings cross as `CStr`, since a zo `str` carries a length header a C function would misread. zo's raylib and sqlite providers are built on these declarations.

Writing one `pub ffi` line per function, kept in sync with the library, is the tedious part, so `zo-binder` writes them for you. Point it at a Rust crate that exports `extern "C"` functions, or at a C library's machine-readable header, and it emits the `#link` block and one `pub ffi` per function, mapping each C type to its zo equivalent and reporting what it cannot. The result is committed and reviewed like any other source; nothing runs during `zo run`.

### generic code.

A function or type can take a type parameter, written with a `$` sigil to set it apart from an ordinary name. `fun max<$T>(a: $T, b: $T) -> $T` runs for any `$T`; `struct Vec<$T>` holds elements of one type fixed per use. Functions, structs, enums, and `apply` blocks all take parameters this way; `Vec`, `Option`, `Result`, and `HashMap` are generic types from the standard library, written with the same parameters your own code uses. The `$T` notation comes from Jai.

zo specializes generics at compile time. Each set of type arguments produces its own code — `Vec<int>` and `Vec<str>` compile to separate, concrete versions, with no boxing and no runtime type tags. Because the compiler knows each `$T`'s size and layout where it emits the code, a generic type lays out exactly like its hand-written equivalent. A type parameter can also be bounded by an abstract: a generic written `<$T: Eq>` accepts only types that implement `Eq`, which guarantees the operations its body uses.

### abstracts.

Generics give one body of code that runs over many types; abstracts give one name to behavior that many types implement differently. The first is parametric polymorphism, the second ad-hoc polymorphism.

An abstract is a contract — a set of method signatures a type can promise to keep. `abstract Display { fun display(self) -> str; }` declares one; the methods take `self`, and `Self` names the implementing type. A type keeps the contract with an `apply` block: `apply Display for Point { fun display(self) -> str { … } }`.

zo ships `Eq`, `Ord`, and `Show`, wired into the language: `==` and `!=` dispatch to `Eq`'s `eq`, `sort_by` and `min_of` to `Ord`'s `cmp`, and `showln` to `Show`, so any type that applies `Show` can print itself.

A function reaches an abstract in one of three ways, trading speed for code size. Written `item: Display`, the compiler specializes a copy of the function per concrete type, with no indirection. Written `<$T: Display>(item: $T)`, it specializes the same way but names the parameter, so you can reuse it across the signature or give it more than one bound at once — `<$T: Show + Eq>`. Written `item: any Display`, the value is boxed behind a vtable: one copy of the function serves every type, a `[]any Display` can hold a mix of them, and each call pays a runtime lookup.

Abstracts are typeclasses, in the line of Haskell and Rust's traits.

### local type inference.

zo asks for types at a function's boundary and infers them inside it. Every parameter names its type, and a function that returns a value names that too. A local binding takes its type from its initializer instead: `imu n := xs.len()` is an `int` because `len` returns one, and the inferring form `:=` stands in for the explicit `imu n: int = xs.len()`.

Inference stays local. zo never works backward from a body to guess a parameter or return type — a signature is something you write, and the compiler checks the body against it. Within a body, types flow by unification, the technique Hindley-Milner is built on.

### metaprogramming.

zo's compiler is an interpreter: it runs your code at compile time. The pass that checks types is the same one that executes declarations and constant expressions, emitting SIR as it goes — the execution-based model zo takes from Carbon. So metaprogramming in zo happens at build time; there is no runtime reflection and no runtime code generation, because by the time a program runs every structure has already been resolved.

Directives are where you reach this directly. A directive is `#` followed by a name and an argument: `#render view` lowers a template to rendering instructions, and `#link { … }` binds a foreign library. The compiler executes each one while it builds SIR, so a directive generates code without a separate macro or preprocessor pass.

Derivation works the same way. Mark a struct `%% serialize` and the compiler reads its fields and writes its `to_json` and `from_json`, composing through nested types with no glue by hand. Reading a type's structure to generate code is reflection — in zo it runs in the compiler, not the program.

### errors and failure.

zo separates what can go wrong into two kinds. An error you might handle is a value: a function that can fail returns `Result<T, E>`, and one that might come back empty returns `Option<T>`. You read either with `match`, so the failing path shows up in the code and in the type. zo has no exceptions.

A failure you cannot handle is a panic. `panic` ends the task it runs in — not the whole program — and carries a payload that says what happened. zo does not unwind the stack; the task stops where it is, and there is no resuming it.

A panic does not vanish. A task that awaits a panicked task receives the panic in turn. A `nursery` holds a child's panic at its boundary; a `supervise` scope lets it cascade, cancelling the siblings and travelling past the scope. This is Erlang's "let it crash": a failing task dies instead of recovering in place, and its failure is handled at the boundary that contains it.

### static control over mutability.

A binding is immutable unless you say otherwise. `imu x: int = 0` names a value you cannot reassign; `mut x: int = 0` names one you can. The same mark carries into signatures: a parameter or method receiver that a function changes is written `mut`, so the outside sees what a call can alter.

Every value has one owner. Hand an owned value to something that consumes it — a `File`'s `close(own self)`, say — and the old binding is spent; using it again is a compile error.

When an owner leaves scope, zo frees what it held, so there is no garbage collector and, in the common case, nothing to free by hand. The compiler checks that a value is not used after it moves and that it is freed exactly once; it does not borrow-check the way Rust does. This safe-manual-memory model comes from Cyclone and C++'s deterministic destructors.

### helpful error messages.

zo builds each compiler error as an argument. Most compilers give you only the conclusion — the bare claim — and leave you to find the reasoning on Stack Overflow. A zo diagnostic carries the whole argument: the claim (the rule you broke), the grounds (the evidence in your code), and, when the fix is mechanical, a resolution. Friendly compiler errors were Elm's idea, from Evan Czaplicki; zo takes the argument structure from Toulmin, after Barik and colleagues showed that the answers developers go looking for are arguments while most compiler errors are not.

The claim leads. "expected X, found Y" is only a ground — a fact with no judgment attached — so zo states the rule first ("type mismatch", "cannot mutate an immutable variable") and shows the conflicting values beneath it. The carets point at those values, never at the operator that joined them: `42 ++ "hello"` underlines `42` and `"hello"`, the two whose types disagree, each labelled with its type. A diagnostic always points at real source; it is never raised without a location.

A fix and an explanation are kept apart. When the repair is mechanical — `mut` before a binding, a missing `;` — zo attaches it as an edit a tool can apply, not as prose to read. The deeper "why" waits behind `--explain-decisions` until you ask for it.

The same diagnostic renders for whoever reads it. By default it is a colored snippet on stderr; `--format json` and `--format xml` emit the identical content as structured data on stdout, so an editor or an agent acts on the `id`, the span, and the fix without parsing prose.

## influences.

> *"Why accept slow compilers? Just make them faster." — Jonathan Blow*

> *"Semantic is king." — Robert Virding*

> *"Simplicity is a prerequisite for reliability." — Edsger W. Dijkstra*

> *"Re-think traditional compiler design." — Chandler Carruth*

> *"The faster your software runs, the less power is consumed." — Chandler Carruth*

> *"Challenge assumptions with aggressive goals." — Chandler Carruth*

> *"Performance is king!" — Mike Acton*

> *"People don't get it! People don't know how fast computers are!" — Jonathan Blow*

---

zo borrows more than it invents. Each of its ideas was proven first in another language — some decades old, some still in active use — and what zo adds is the combination. These are the lineages behind it:

**-rust-prehistory**

Rust (2006), created by Graydon Hoare — the original, pre-1.0 design preserved as the "rust-prehistory" compiler that names this lineage.

**-carbon**

Carbon (2022), Google's experimental successor to C++, introduced by Chandler Carruth.

**-jai**

Jai (2014), Jonathan Blow's systems language for games, long in development. With no published specification, it is known through Blow's two video series — "A Programming Language for Games" and the "Compiler programming livestreams".

**-erlang**

Erlang (1987), created by Joe Armstrong, Robert Virding, Claes Wikström, Mike Williams, and others at the Ericsson Computer Science Laboratory in Älvsjö, Stockholm.

**-newsqueak-and-go**

Newsqueak (1988), Rob Pike's concurrent language at Bell Labs; and Go (2009), the systems language designed by Robert Griesemer, Rob Pike, and Ken Thompson at Google.

**-cyclone-and-ada**

Cyclone (2002), a memory-safe dialect of C, by Trevor Jim, Greg Morrisett, Dan Grossman, Michael Hicks, and others at AT&T Labs Research and Cornell University; and Ada (1983), designed by Jean Ichbiah's team at CII Honeywell Bull for the US Department of Defense, and named after Ada Lovelace.

**-imba**

Imba (2015), a web programming language created by Sindre Aarsæther.

**-e4x-and-es4**

E4X (ECMA-357, 2004), an extension that put XML literals directly into JavaScript, designed by Terry Lucas and John Schneider; and ECMAScript 4, the language's fourth edition, abandoned in 2008.

**-elm**

Elm (2012), Evan Czaplicki's purely functional language for building web interfaces, written as his thesis project.

Other languages lent single features rather than whole designs:

  - The algebraic data types of SML.
  - The macro system of Clojure.
  - The deterministic destructor system of C++.

## tutorial.

See the [initiation](https://zo.compilords.house/initiation)

## reference.

  - Lexical structure.
  - Grammar.
  - zsx.

### lexical structure.

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

#### ignored characters.

Whitespace and comments lie between tokens and are not tokens themselves; the tokenizer discards both, and zo is not whitespace-sensitive.

Whitespace is any of U+0020 (space), U+0009 (tab, `\t`), U+000A (line feed, `\n`), and U+000D (carriage return, `\r`).

```ebnf
whitespace = ? space | tab | newline | carriage return ? ;
```

A comment is one of three forms, recognized only outside string, character, and template literals:

  - a line comment runs from `--` to the next line feed;
  - a doc comment runs from `-!` to the next line feed, its text meant as documentation;
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

#### identifier tokens.

An identifier begins with a letter or underscore (U+005F) and continues with any combination of letters, digits (U+0030–U+0039), and underscores. It may not be spelled the same as a keyword.

A letter is currently ASCII: U+0041–U+005A and U+0061–U+007A (`A`–`Z`, `a`–`z`). The tokenizer scans identifiers a byte at a time for speed, so it does not yet accept non-ASCII letters — Unicode identifiers, which the grammar below already admits, are a planned extension.

```ebnf
identifier = ( letter | "_" ), { letter | digit | "_" } ;
letter = ? 'a'..'z' | 'A'..'Z' | unicode letter ? ;
digit  = ? '0'..'9' ? ;
```

#### keyword tokens.

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

#### operators and punctuation.

Operators bind from tightest to loosest as listed; the `when … ? … :` ternary is looser than all of them.

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

Assignment writes back to an existing place; the binding operators introduce a new name instead — `:=` infers the type, `::=` binds a template.

```ebnf
assign_op = "=" | "+=" | "-=" | "*=" | "/=" | "%="
          | "<<=" | ">>=" | "&=" | "|=" | "^=" ;
```

A few operators carry their own meaning: `->` gives a return type, `=>` a match arm or closure body, `=:>` a template-returning closure, `|>` a reserved pipe, `::` a path separator, `?` the ternary's second half, and `@` a zsx event attribute.

Delimiters pair an opener with a closer — `( )` parentheses, `{ }` braces, `[ ]` brackets — and `<> … </>` wraps a template fragment.

Punctuation: `,` separates items, `;` ends a statement, `:` ascribes a type, `_` is the wildcard, `%%` introduces an attribute, and `...` is the spread. `#` begins a directive and `$` prefixes a type parameter, raw string, or style block.

#### integers literals.

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

#### floating-point literals.

A float literal places a decimal point between two runs of digits, carries an exponent, or both. An exponent is `e` or `E`, an optional sign, and decimal digits: `3.14`, `6.022e23`, `1.0e-9`.

```ebnf
float_literal = decimal_literal, ".", decimal_literal, [ exponent ]
              | decimal_literal, exponent ;
exponent      = ( "e" | "E" ), [ "+" | "-" ], decimal_literal ;
```

#### string and character literals.

A string literal is double-quoted UTF-8 text, and a string is immutable. A character literal is a single Unicode character between single quotes — `'a'`, `'🙂'` — of type `char`. Both accept escape sequences:

  - `\n` `\r` `\t` `\\` `\"` `\'` `\0` — newline, carriage return, tab, backslash, the two quotes, and null;
  - `\a` `\b` `\e` `\f` `\v` — bell, backspace, escape, form feed, and vertical tab;
  - `\xHH` — a byte from two hex digits, as in `\x48`;
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

#### bytes literals

A bytes literal is delimited by backticks and evaluates to a `bytes` value — a byte buffer, not a UTF-8 string. It accepts the same escape sequences as a string literal but does not interpolate, so `\x48\x65` is two bytes.

```ebnf
bytes_literal = "`", { bytes_char }, "`" ;
bytes_char = char_escape | ? any unicode character except '`', '\' and newline ? ;
```

#### types.

...

##### boolean types.

The boolean type is `bool`, whose only values are the literals `true` and `false`.

```ebnf
boolean_type = "bool" ;
bool_literal = "true" | "false" ;
```

##### numeric types.

The default integer is `int`, a signed 32-bit value; `uint` is its unsigned counterpart. The fixed-width integers are `s8`, `s16`, `s32`, `s64` and `u8`, `u16`, `u32`, `u64`. The default floating-point type is `float`, a 64-bit double, beside the fixed widths `f32` and `f64`.

```ebnf
numeric_type = integer_type | float_type ;
integer_type = "int" | "uint" | "s8" | "s16" | "s32" | "s64"
             | "u8" | "u16" | "u32" | "u64" ;
float_type = "float" | "f32" | "f64" ;
```

##### string types.

`str` is an immutable UTF-8 string, `char` is a single Unicode scalar, and `bytes` is a raw byte buffer.

```ebnf
text_type = "str" | "char" | "bytes" ;
```

##### array types.

An array type wraps an element type in brackets: `[N]T` is a fixed array of `N` elements, and `[]T` is a dynamically sized one.

```ebnf
array_type = "[", [ int_literal ], "]", type ;
```

##### function types.

A function type is written `Fn(…) -> T`: the parameter types in parentheses, then the return type after the arrow.

```ebnf
function_type = "Fn", "(", [ type, { ",", type } ], ")", "->", type ;
```

##### abstract types.

...

##### enumeration types.

...

##### struct types.

...

##### block expressions.

...

#### zo syntax extension.

...

#### css and shorthands.

...
