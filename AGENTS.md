# AGENTS.md

> zo — TURN YOUR THOUGHTS iNTO TYPE-SAFE SOFTWARE AND Ui iNSTANTLY.

## Project Overview

**THE zo PROGRAMMiNG LANGUAGE.**

A LANGUAGE TO SHiP, RUN AND BUiLD NATiVE AND WEB APPLiCATiONS OPTiMiZED FOR SPEED. zo iS A FEATURE-RiCH ECOSYSTEM FOCUS ON HiGH-PERFORMANCE, SiMPLiCiTY AND DATA-ORiENTED.

ONE LANGUAGE. ONE COMPiLER. ONE BiNARY. NATiVE OR WEB — SAME SOURCE.

  ```zo
  -! An Ui counter program in zo.

  fun main() {
    mut count: int = 0;

    -- Look! You can use `zsx` to declare Ui.
    imu counter: </> ::= <>
      <button @click={fn() => count -= 1}>-</button>
      {count} <!-- HTML comments in `zsx` -->
      <button @click={fn() => count += 1}>+</button>
    </>;

    #dom counter;
  }
  ```

The grammar can be found here: @crates/compiler/zo-notes/public/grammar/zo.ebnf

## Build & Test

All build commands go through `just` (the justfile is the single source of truth):

  ```sh
  just typos           # Check for typos.
  just fmt             # Format code.
  just clippy          # Clippy with -D warnings.
  just test            # Run all tests across the full workspace (nextest).
  just test_crate X    # Test a single crate.
  just test_filter X   # Test by name filter.
  just check           # Check all workspace crates.
  just build           # Build all targets.
  just bench           # Run all benchmarks.
  just zo_test         # Run unit tests across every `zo*` crate (nextest, package filter).
  just zo_test_runner  # Run `zo-test-runner` — integration on compiled zo programs.
  just pre-commit      # Full pipeline: typos -> fmt_check -> clippy -> test.
  ```

> *@justfile — All recipes available.*

## Code Style

## Testing requirements.

- `just zo_test_runner` must be always green.

## Important

  - THE MANiFESTO DEFiNE OUR ViSiON @crates/compiler/zo-notes/personal/manifesto.md
  - ALWAYS verify to clean all stuff that you add in `/tmp` for debugging. Pollution is BAD.
  - USE THE `check` COMMAND WHEREVER POSSiBLE WHEN YOU NEED TO CHECK THAT THE PROJECT COMPiLES.
  - PERFORMANCE MATTER, WE COMMiT TO NOT DEGRADE COMPiLER BUiLD AND RUNTiME SPEED.

## Guidelines

- https://corrode.dev/blog/defensive-programming
- https://corrode.dev/blog/bugs-rust-wont-catch
- https://stackoverflow.blog/2021/12/23/best-practices-for-writing-code-comments