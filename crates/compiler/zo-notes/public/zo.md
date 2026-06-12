# zo.

> *Turn your thoughts into type-safe software and Ui instantly.*

This manual is for the zo programming language.

@author — invisageable
@author — compilords

> *DiSCLAiMER — zo is in early development. We are opening gates, testing some paths. Changes will be made, certain parts work, certain parts don't. Some of them will be removed.*
> 
> *Some features described in this manual may not be available yet. It crafts the foundation as a draft. This document is not the final specification.*
>
> *For any suggestion, keep your focus on reductions to the language. What feature can be combined or omitted? At this point, every ``additive'' feature we're likely to support is already on the table. The task ahead involves combining, trimming, and implementing.*

## introduction.

zo is a Rust-prehistory's child. It inherits most of its concepts and philosophy. Without being an erzatz, zo is way far from Rust and find its niche with zsx (zo Syntax Extension) as builtin to build cross-platform user interfaces. It's a general-purpose programming language so you can create and maintain any kind of application.

zo is a mix of imperative, concurrent actor, functional styles. It supports generics, foreign function interfaces, ...

## goals.

The language design pursues the following goals:

  - Compile-time error detection and prevention.
  - Run-time fault tolerance and containment.
  - Clarity and precision of expression.
  - Implementation simplicity.
  - Run-time efficiency.
  - High concurrency.

> *NOTE — zo is inspired by technologies that have been used earlier in other languages. These engineering goals are already been prove and are pretty solid. zo do not reivent the wheel, it assembles thing accordingly.*

Like all new langages developped in a technological context. zo's goals focus on writing large programs that interact with internet (server & client), user interfaces and are thus much more concerned with safety and concurrency than older generations of program.

## features.

### no-pointer.

...

### lightweight tasks with no shared mutable state.

...

### ui template as builtin.

...

### cross-platform.

...

### direct interface to C code.

...

### generic code.

...

### local type inference.

...

### dynamic metaprogramming.

...

### static metaprogramming.

...

### idempotent failure.

...

### type inference.

...

### static control over mutability.

...

### helpful error messages.

...

## influences.

> *"Why accept slow compilers? Just make them faster." — Jonathan Blow*

> *"Semantic is king." — Robert Virding*

> *""Simplicity is a prerequisite for reliability." — Edsger W. Dijkstra*

"\"Re-think traditional compiler design.\" — Chandler Carruth",

"\"The faster your software runs, the less power his consumed.\" — Chandler Carruth",

"\"Challenge assumptions with aggressive goals.\" — Chandler Carruth",

"\"Performance is king!\" — Mike Acton",

"\"People don't get it! People don't know how fast computers are!\" — Jonathan Blow",

---

zo is not a particularly original language. It may however appear unusual by contemporary standards, as its design elements are drawn from a number of `historical` languages that have, with a few exceptions, fallen out of
favour. Five prominent lineages contribute the most:

**-rust-prehistory**

...

**-jai**

...

**erlang**

...

**-cyclone-and-ada**

...

**-imba**

...

**-es4-and-e4x**

...

Additional specific influences can be seen from the following languages:

  - The structural algebraic types and compilation manager of SML.
  - The syntax-extension systems of Camlp4 and the Common Lisp readtable.
  - The deterministic destructor system of C++.

## tutorial.

See the [initiation](https://zo.compilords.house/initiation)

## reference.

  - Lexical structure.
  - Grammar.
  - zsx.

### lexical structure.

...

  - Ignored characters.
  - Identifier tokens.
  - Keyword tokens.
  - Numeric tokens.
  - String and character tokens.
  - Syntactic extension tokens.
  - Special symbol tokens.

**-ignored-characters**

...

**-identifier-tokens**

...

**-keyword-tokens**

...

**-numeric-tokens**

...

**-string-and-character-tokens**

...

**-syntactic-extension-tokens**

...

**-special-symbol-tokens**

...

### grammar.

See the EBNF [grammar](crates/compiler/zo-notes/public/grammar/zo.ebnf).

### zsx.

...

