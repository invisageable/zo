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
  - idiomatic Rust and reuse of existing infrastructure

## Guidelines

- https://corrode.dev/blog/defensive-programming
- https://corrode.dev/blog/bugs-rust-wont-catch
- https://stackoverflow.blog/2021/12/23/best-practices-for-writing-code-comments

Add at the very top of CLAUDE.md under a ## Core Principles section — this is the single most important behavioral rule.\n\nNEVER take shortcuts or apply workarounds. Always diagnose the root cause before proposing a fix. Do not apply speculative patches — understand WHY something is broken first, then fix it properly.
Add under ## Core Principles or ## Git Rules section.\n\nNEVER commit to git unless explicitly asked. Do not run `git commit` autonomously.
Add under ## File Organization section.\n\nFollow project directory structure strictly. Never place files in /tmp, never create files in wrong directories. When unsure about where a file belongs, check existing crate/module structure first — do not guess.
Add under ## Zo Project Conventions section.\n\nRespect zo project conventions:
- Use the project's reporter pipeline for diagnostics, NEVER eprintln/println
- Use Option<T> instead of empty string sentinels
- Follow FFI naming conventions already established in the codebase
- Do not write narration-style or explanatory comments in code
- Do not add `#[allow(...)]` to suppress clippy warnings — fix the underlying issue
- Do not import zo-executor in compiler crates
- Do not add phases/variants to enums unless architecturally justified
Add under ## Testing section.\n\nAlways run the full integration test suite (`cargo test`) before declaring a task complete. Never claim success without test evidence. If tests fail, diagnose and fix — do not move on.
Add under ## Skills / Commands section.\n\nWhen asked to 'simplify' code, this means reduce complexity while preserving identical behavior — it does NOT mean rewrite, remove features, or refactor architecture. Read the skill definition carefully before acting.
Add under ## Zo Project Conventions section.\n\nDo not reorder user code to fix compiler bugs. The compiler must handle code in any order — fix the compiler, not the user's source.