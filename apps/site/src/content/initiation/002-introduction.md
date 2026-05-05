---
order: 3
---

# introduction

This guide is your initiation to zo. Each chapter builds on the previous one — read in order on a first pass, jump around once you know the basics.

## how to use this guide

Every lesson is a quick snapshot of a zo program. Read the doc-comments (`-!`), they provide explanation and describe all the concepts that you need.

  ```zo
  -! Hey, I'm a doc comment, read me to understand
  -! what's going on in the snapshot program.
  ```

  ```zo
  -- And me I'm a line comment, mostly introduced within the program.
  ```

  ```zo
  -*
  From my side I'm a block comment, I'm happy to help for details that matter
  *-
  ```

<!-- roadmap of the book -->

In zo, each program has to contain an entry point:

  ```zo
  -- `fun` declares a function. 
  -- `main` is the entry point — the first piece of code that runs.
  fun main() {
    -- This program does nothing... yet.
  }
  ```

That's it. Turn the page.
