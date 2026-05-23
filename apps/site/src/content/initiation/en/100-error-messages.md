# error messages

- show error messages per severity:
  - warning - not blocking compilation
  - error - blocking compilation
  - agentic error messages `--format=json`
  ```zo
  -- Let's presume that you or your Ai did not declare
  -- an entry point
  ```

The compiler includes structural formatting directives designed for machine integration pipelines using `--
format=json` flags.

  ```zo
  imu pass: Result = Result::Pass("payload data");
  -- Fast unwrapping operations using the short-circuit bubbling '?' postfix
  imu verified_bytes = read_file(path)?.validate()?;
  ```

## abstract diagnostics.

### E0347 — `BoundNotSatisfied`.

A generic call site passes a concrete type that does not
implement the abstract bound declared on the generic
parameter. Fires for both forms of bound:

  ```zo
  fun render(item: Show) -> str {
    item.show()
  }

  struct Unimpl { payload: int }

  fun main() {
    imu u: Unimpl = Unimpl { payload = 42 };

    -- Unimpl has no `apply Show for Unimpl`.
    showln(render(u));
    -- ^^^^^^^^^^^ E0347 BoundNotSatisfied
  }
  ```

The diagnostic carries two spans: the primary points at the
offending argument, the secondary points at the abstract
bound declared on the function's signature so the user
sees both ends of the mismatch.

**Fix:** add `apply Show for Unimpl { fun show(self) -> str { ... } }`,
or pass a value of a type that already implements the
abstract.

### E0348 — `AbstractInheritanceUnsupported`.

`abstract Cmp : Eq { ... }` colon-after-abstract-name syntax
is rejected. zo abstracts are flat single-level
declarations; inheritance would force vtable lookups to
chain through a parent table.

**Fix:** drop the `: Eq` clause and add a parallel
`apply Eq for Type` block alongside the `apply Cmp for Type`.

### E0349 — `AbstractNotDynSafe`.

The `any <Abstract>` annotation referenced an abstract that
isn't safe for dynamic dispatch. A method's signature uses
`Self` outside the receiver position:

  ```zo
  abstract Combine {
    fun merge(self, other: Self) -> Self;
    --                          ^^^^   ^^^^
    --              Self past receiver position —
    --              uniform vtable calling convention
    --              has no slot for "another instance
    --              of the same concrete type".
  }

  fun reduce(item: any Combine) -> int {
    --             ^^^^^^^^^^^ E0349 AbstractNotDynSafe
    0
  }
  ```

**Fix:** swap `any Combine` for `item: Combine` (form 1) or
`<$T: Combine>(item: $T)` (form 2) — both monomorphize per
concrete type and don't have the uniform-call constraint.
Or drop the `Self`-using method so the abstract becomes
dyn-safe.
