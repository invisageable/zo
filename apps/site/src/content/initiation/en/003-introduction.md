---
order: 3
---

# introduction

This guide is your initiation to the zo programming language ecosystem. Read sequentially on your first pass, then skip around once you grasp the architectural patterns.

## how to use this guide

Every lesson delivers a high-fidelity snapshot of a functional zo program. Pay close attention to the comments: documents comments (`-!`) leverage raw Markdown formatting to establish systemic guidelines, while line (`--`) and block (`-* *-`) markers isolate execution mechanics.

  ```zo
  -! Sup? I'm a doc comment, I support markdown
  -! format. What's good?
  ```

  ```zo
  -- I'm a line comment, I don't care about markdown.
  ```

  ```zo
  -*
    From my side I'm a block comment, I'm happy to help
    for details that matter.
  *-
  ```

Every executable compilation unit inside zo must expose an explicit, non-colored entry block called `main`:

  ```zo
  -- Wassup?! I'm `main` a function.
  -- Use me as a entry point with `fun` keyword.
  fun main() {
    -- This program does nothing... yet.
  }

  -! ## the capstone.
  -!
  -!   - every programs must declare a `main` block
  -!     to serve as the runtime launchpad.
  ```
