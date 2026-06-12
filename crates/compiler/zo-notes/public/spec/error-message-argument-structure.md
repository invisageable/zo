# Error Messages as Arguments

How zo structures compiler diagnostics, based on Barik et al., *"How Should Compilers Explain Problems to Developers?"* (ESEC/FSE 2018).

The paper studies why developers leave a compiler error and search Stack Overflow instead. The finding: accepted Stack Overflow answers are **arguments** (Toulmin's model) — claim, grounds, warrant, resolution. Most compiler errors are a bare **claim** with nothing behind it. We adopt the argument model so a zo error stands on its own and the user never has to leave.

## Table 4 — Argument Layout Components

The components every diagnostic can carry. Simple components form the
baseline; extended components are opt-in depth.

### Simple argument components

| Component      | What it is                                                        |
|----------------|-------------------------------------------------------------------|
| **Claim**      | The concluding judgment about the problem in the code.            |
| **Resolution** | A concrete action on the source that fixes the problem.           |
| **Grounds**    | Facts, rules, and evidence that support the claim.                |
| **Warrant**    | The bridge — *why* the grounds support the claim ("because …").   |

### Extended argument components

| Component     | What it is                                                         |
|---------------|-------------------------------------------------------------------|
| **Backing**   | Extra evidence for the warrant, when the warrant isn't accepted.  |
| **Qualifier** | Degree of belief in the claim ("likely", "probably") — weakens it.|
| **Rebuttal**  | Exceptions to the claim or to another component.                  |

## The four layouts (Figure 3)

Diagnostics fall into four shapes. The paper measured how often each appears in real compiler errors (CEM) versus accepted answers (SO):

  - **(a) Claim-only** — claim, nothing else. *Compilers: 191. SO: 0.* The problematic default. Avoid.
  - **(b) Claim-resolution** — claim + fix. Right when the fix is obvious (a missing `;`).
  - **(c) Simple argument** — grounds + warrant ⇒ claim, plus resolution. The target for judgment-call errors (type mismatches, ownership).
  - **(d) Extended argument** — adds backing. Reserve for opt-in depth.

The lesson: **never ship layout (a)**. Every zo diagnostic carries at least a resolution or grounds.

## Three design principles (we apply all three)

**I — Give the user autonomy to elaborate the argument.**

Novices want the explanation; experts want the fix. Ship a *simple* argument by default and let the user pull *backing* on demand. zo already gates depth: `--explain-decisions` opens the rationale channel, `--format json` exposes the full structure for tools and agents.

**II — Distinguish fixes from explanations.**

A *resolution* (quick fix) and an *argument* (explanation) are different things. A missing semicolon needs only the fix. A type mismatch needs the grounds and warrant — the fix alone leaves the user not knowing *why*. Keep the two channels separate; don't collapse a fix into prose or bury a fix inside an explanation.

**III — Use argument structure to design and review errors.**

When writing or reviewing a diagnostic, name each component. If you can't point at the claim, the message is incomplete.

## The trap: a ground masquerading as a claim

The paper's sharpest lesson. Haskell's `ghci` on `[True, 'a']`:

```
Couldn't match expected type 'Bool' with actual type 'Char'
```

That is a **ground**, not a claim — it states a fact without the judgment it supports. F# states the **claim** first, then backs it:

```
error FS0001: All elements of a list constructor expression must have the same type. This expression was expected to have type 'bool', but here has type 'char'.
              ^^^^
```

F# also points at the location with a caret; `ghci` narrates it through "In the expression …" lines. Carets beat narration.

**Apply to zo:** lead `TypeMismatch` and friends with the *rule that was violated* (the claim), then present the conflicting types as grounds. Don't open with "expected X, found Y" — that's a ground.

## How the components map onto zo's reporter

zo's `Error` is a compact 16-byte value (kind + primary span + secondary span + file). The `ErrorKind` is the key into static side-tables that supply the rest of the argument — zero cost on the hot path, full structure when rendering:

| Argument component | zo mechanism                                                      |
|--------------------|-------------------------------------------------------------------|
| **Claim**          | `ErrorKind` headline + primary span (ariadne caret on the source).|
| **Grounds**        | Secondary span (the conflicting site) + the concrete values.      |
| **Warrant**        | `error_help(kind)` — the "because …" prose in `zo-reporter`.      |
| **Resolution**     | `fixes_for(kind)` — machine-applicable `FixIt` edits.             |
| **Backing**        | Rationale channel, gated by `--explain-decisions`.                |
| **Qualifier**      | Avoid. A compiler asserts; it does not hedge with "probably".     |
| **Rebuttal**       | Rare. Use only for genuine, documented exceptions.                |

The infrastructure already exists. The work is **content**: making each `ErrorKind` carry a true claim, real grounds, and — for judgment-call errors — a warrant.

## A real zo diagnostic, decomposed

Compiling this program:

```zo
fun main() {
  imu count: int = 0;
  count = 1;
}
```

produces:

```
[E0309] Error • Cannot mutate immutable variable
   ╭─[ immutable.zo:3:3 ]
   │
 3 │   count = 1;
   │   ──┬──
   │     ╰──── cannot assign to immutable variable
   │
   │ Help • Use 'mut' to declare a mutable variable
───╯
```

Reading it through Table 4:

| Component      | In this diagnostic                                                       |
|----------------|--------------------------------------------------------------------------|
| **Claim**      | `[E0309] Cannot mutate immutable variable`.                              |
| **Grounds**    | The source line + caret under `count`, labelled "cannot assign …".       |
| **Resolution** | `Help • Use 'mut' to declare a mutable variable`.                         |
| **Warrant**    | *Missing* — the *why* (`imu` binds immutably) is left implicit.          |

This is a **claim-resolution** layout (b): a clear claim, a caret on the grounds, and a fix. The gap is the **warrant** — it never says *because `count` was bound with `imu`, not `mut`*. For a one-keyword fix that's acceptable; for judgment-call errors it isn't, and the warrant must be stated.

`--format json` emits the same structure machine-readably — `code`, `message`, `span`, `fixes` (the `FixIt`), `notes`, plus `secondary` / `primary_type` / `secondary_type` when a diagnostic names two conflicting values (a type mismatch) — so an agent applies the resolution without parsing prose:

```json
{ "id": "immutable-variable", "code": "E0309", "severity": "error",
  "message": "Cannot mutate immutable variable",
  "fixes": [ { "kind": "insert", "text": "mut ",
    "description": "Declare the variable as mutable with `mut`" } ] }
```

## Why locations matter: the `Span::ZERO` rule

A claim with no location is a degraded claim — the user can't see what the compiler is pointing at, the very failure F# fixes with its caret. So **a user-facing diagnostic must never be built with `Span::ZERO`**.

`Span::ZERO` is legitimate only for synthetic nodes and for `InternalCompilerError` sentinels (a compiler bug has no user source location). Any other diagnostic must thread a real span from the tree.

### A type mismatch highlights the conflicting values

The grounds for a type mismatch are the *values whose types disagree* — so every `TypeMismatch` points its carets at those values, never at the operator or keyword that joined them. A caret on `++` or `if` is a ground masquerading as a claim: it names the operation, not the evidence.

For `42 ++ "hello"`:

```
[E0304] Error • Type mismatch
   ╭─[ concat.zo:2:16 ]
   │
 2 │   imu s: str = 42 ++ "hello";
   │                ─┬    ───┬───
   │                 ╰───────────── incompatible type `int` here
   │                         ╰───── conflicts with this type `str`
   │
   │ Note • The types of both operands must be compatible
───╯
```

Both values are lit *and named*: `42` (`int`, primary) and `"hello"` (`str`, secondary). The same holds for branch arms (`when c ? 1 : true`), logical operators (`true || "false"`), and a function body that contradicts its return type (`fun main() -> str { "DONE" }` points at `"DONE"`, typed `str`).

How it works, with zero happy-path cost:

- Each value's source span is recovered only on the error path. `Sir::node_of_value` finds the value's defining instruction and reads back its parse-node span — no per-value bookkeeping during normal execution.
- `TyChecker::unify_silent` runs the unification without self-reporting, so the executor owns the diagnostic and emits both spans via `report_value_mismatch` (primary = offending value, secondary = the value it conflicts with).
- The conflicting type names are the first *dynamic* diagnostic data. The compact 16-byte `Error` can't hold strings, so the executor resolves each value's type to a name and reports it via `report_error_with_types`; the names ride to the renderer as `TyNames` keyed by the `Error`, surfacing in the labels and as `primary_type` / `secondary_type` in JSON.
- The operator / construct span is only a fallback when a value has no recoverable span (a synthetic with no defining instruction).

## Checklist for any new diagnostic

  1. **State the claim** — the rule that was violated, not a raw fact.
  2. **Point at it** — a real span, never `Span::ZERO`. Caret, not prose.
  3. **Give grounds** — the conflicting values / the second span.
  4. **Add a warrant** (`error_help`) when the *why* isn't obvious.
  5. **Offer a resolution** (`FixIt`) when the fix is mechanical.
  6. **Keep backing opt-in** — depth behind `--explain-decisions`.
  7. **Never ship claim-only.** At minimum: claim + resolution, or claim + grounds.

## Most common errors:

Across languages, the errors developers hit cluster into a handful of buckets. Ranked roughly by how often people actually encounter them:

Syntax / parse (the daily ones)

  - Missing delimiter — `;`, `)`, `}`, `]`. The single most frequent compile error.
  - Unexpected token — "expected X, found Y".
  - Unterminated string / comment.
  - Unexpected end of file.

Name resolution (usually typos)

  - Undefined variable — NameError (Python), ReferenceError: x is not defined (JS), "cannot find value x in this scope" (Rust).
  - Undefined function / type / module.
  - Use before declaration.

Type errors (the conceptual ones)

  - Type mismatch — "expected int, found str". The headline type error everywhere.
  - Argument count mismatch — too few / too many arguments.
  - "X is not a function" / not callable.
  - Missing return / not all paths return a value.
  - Operator not applicable to these types (e.g. int + str).

The runtime "big four" (these dominate production crashes)

  1. Null / nil / undefined dereference — NullPointerException (Java), "cannot read properties of undefined" (JS), AttributeError: 'NoneType' (Python), nil-pointer panic (Go). Hoare's "billion-dollar mistake" — by far the #1 runtime failure across the industry.
  2. Index out of bounds — array/list/string.
  3. Division by zero.
  4. Stack overflow (infinite recursion) / out of memory.

Ownership / lifetimes (Rust's signature class)

  - Use after move, cannot borrow as mutable, borrowed value does not live long enough.

## Source

Titus Barik, Denae Ford, Emerson Murphy-Hill, Chris Parnin. *How Should Compilers Explain Problems to Developers?* ESEC/FSE 2018. <https://static.barik.net/barik/publications/fse2018/barik_fse18.pdf>
