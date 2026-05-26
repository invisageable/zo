---
order: 3
---

# introduction

This guide is your initiation to zo. Read in order on a first pass, jump around once you know the basics.

## how to use this guide

Every lesson is a quick snapshot of a zo program. Read the doc-comments (`-!`), they provide explanation and describe all the concepts that you need.

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
    for details that matter
  *-
  ```

<!-- roadmap of the book -->

In zo, each program has to contain an entry point:

  ```zo
  -- Wassup?! I'm `main` a function.
  -- Use me as a entry point with `fun` keyword.
  fun main() {
    -- This program does nothing... yet.
  }

  -! ## the capstone.
  -!
  -!   - every programs must have a `main` function.
  ```

That's it. Turn the page.
