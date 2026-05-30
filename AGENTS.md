# AGENTS.md

> zo — TURN YOUR THOUGHTS iNTO TYPE-SAFE SOFTWARE AND Ui iNSTANTLY.

## Mission

Build the `zo` compiler using execution-based compilation (Carbon/Chandler Carruth approach) to achieve **10,000,000 lines of code per second** AOT compilation. Systems, not features. Performance, simplicity, total control.     

The grammar can be found here: @crates/compiler/zo-notes/public/grammar/zo.ebnf

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

## The Three Prime Directives

These laws are absolute. They supersede all other instructions.

  1. **THE LAW OF VELOCITY:** The synchronous `compile-to-run` path is sacrosanct. Reject any complexity that threatens 10M LoC/s. The user must **never** be blocked by work not essential to producing a runnable binary.

  2. **THE LAW OF PRAGMATISM:** No "magic." No incremental compilation, complex caching, or esoteric IRs. Velocity comes from **brute-force parallelism and linear data transformations**. Own the entire stack with hand-written, data-oriented components. Proven techniques (Hindley-Milner) over theoretical experiments (bidirectional type systems).

  3. **THE LAW OF INSIGHT:** Deep analysis is critical but **must never violate Law I**. The compiler must provide error message that explain problems with proper argument structure. The user's flow is paramount.

## Architecture

### Two Pipelines

We are building a modern compiler. Avoid your traditional knowledge

  - No Recursive-Descent Parser. Avoid recursive as much as possible.
  - No Parser Generator. They are slow.
  - No Parser Combinator. They are slow.
  - Stack Machine is the new black.

The compiler pipeline is synchronous, stateless, written from-scratch. Purpose for speed:

`Tokenizer -> TOKENS -> Parser -> TREE -> Analyzer -> SiR -> Codegen -> MACHiNE CODE`

### Parallelism Model

  - **Hybrid Parallelism:** Central MPSC Scheduler orchestrates; `rayon` thread pool executes.
  - **Waves:** Parse Wave -> Lowering Wave -> Codegen Wave. A wave completes before the next begins.
  - All inter-thread data must be `Send + Sync`.

### Data Sovereignty

  - All processes are data transformations: `Source Text -> Tokens -> Tree -> SIR -> Machine Code`.
  - **SIR** (Semantic IR) is the most critical artifact — typed, optimized output of executing Tree.
  - **Tree** (Post-order Tree) is the parse tree — simple, fast, no types, no analysis. Exists only to be executed into SIR.
  - Favor stack allocation, arenas, and zero-allocation strategies (especially tokenizer/parser).

## Execution-Based Compilation Protocol

We do **NOT** use traditional AST -> TypeCheck -> IR phases.

  - **Pipeline:** `Parse -> Tree (parse tree) -> Execute/Analyze -> SIR (semantic IR) -> Codegen`
  - **Core concept:** Semantic analysis is **compile-time execution** that produces IR, not tree annotation.
  - Type checking happens **during** SIR building, not as a separate pass.

### Directives

  1. **Execution model, not tree walking.** The analyzer "executes" Tree to produce SIR. Declarations are compile-time operations. Type checking is evaluation.

  2. **Linear data flow.** Tree nodes processed sequentially as an array. Types flow through a stack machine. SIR emitted immediately as types resolve.

  3. **Single pass semantics.** Tree executed once to produce typed SIR. No multiple tree walks.

    ```rust
    // NOT THIS: walk tree -> annotate -> walk again -> generate IR
    // BUT THIS: execute Tree -> emit SIR (with types) in one pass
    ```

  4. **Streaming architecture.** Can start emitting SIR without complete type information. Types resolved incrementally. Functions execute independently.

### Performance Targets

| Phase                           | Target        | Benchmark  |
|---------------------------------|---------------|------------|
| Tokenize + Parse (-> Tree)      | **10M LoC/s** | Carbon: 8M |
| Semantic Analysis (Tree -> SIR) | **5M LoC/s**  | Carbon: 1M |
| Codegen (SIR -> machine code)   | **5M LoC/s**  | excl. Clif |

## Build & Test

All build commands go through `just` (the justfile is the single source of truth). Some command are missing? DO NOT HESiTATE TO ADD THEM:

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

  - **Indentation**: 2 spaces (no tabs)
  - **Line width**: 80 characters max — see @rustfmt.toml
  - **File operations**: Exclude read/write in `tmp/` folder
  - **Issues**: Always write an integration/unit test in the specific crate
  - **Bug fixes**: Never mark fixed without testing the solution
  - **Warnings**: Never ignore — they often indicate bugs or incomplete code
  - **Root causes**: Fix the cause, not silence the symptom. no shortcuts
  - **Ownership**: If a unit/integration test don't have an equivalent or do not exist. Implemet it
  - **Understand before removing**: Know why code exists before deleting it
  - **Focus on details**: Use an agent without code review is not allowed
  - **Consistency**: Avoid magic numbers

> *Before plan or implementation, use `karpathy-guidelines` skill.*

## Important

  - THE MANiFESTO DEFiNE OUR ViSiON @crates/compiler/zo-notes/personal/manifesto.md
  - ALWAYS verify to clean all stuff that you add in `/tmp` for debugging. Pollution is bad
  - USE THE `check` COMMAND WHEREVER POSSiBLE WHEN YOU NEED TO CHECK THAT THE PROJECT COMPiLES
  - PERFORMANCE MATTER, WE COMMiT TO NOT DEGRADE COMPiLER BUiLD AND RUNTiME SPEED
  - DO idiomatic Rust and REUSE of existing infrastructure
  - This is the single MOST important behavioral rule. NEVER take shortcuts or apply workarounds. ALWAYS diagnose the root cause before proposing a fix. Do not apply speculative patches — understand WHY something is broken first, then fix it properly
  - NEVER commit to git unless explicitly asked. Do not run `git commit` autonomously
  - FOLLOW project directory structure STRiCTLY. NEVER create files in wrong directories. When unsure about where a file belongs, CHECK existing crate/module structure first — DO NOT GUESS
  - FOLLOW FFI naming conventions already established in the codebase
  - DO NOT violate our principles such as DRY and KiSS
  - DO NOT ADD `#[allow(...)]` to suppress clippy warnings — FiX the underlying issue

## Comments & Documentation.

DO NOT write narration-style or explanatory comments in code. Here are the recommendations from Eva Parish:

  - DECiDE What you're actually saying
  - Simplify
  - Eliminate passive voice
  - DON'T USE adverbs
  - DON'T ASSUME knowledge
  - Be AWARE of your TONE
  - Avoid JARGON and CLICHES
  - Make use of whitespace

Any doubt? Fetch [this](https://eva-parish.squarespace.com/blog/how-i-edit) and apply it. Do the same for [this](https://stackoverflow.blog/2021/12/23/best-practices-for-writing-code-comments)

Every new/update feature should be documented in the zo [website](apps/site/src/content/initiation/en).

Doc comments are important in Rust because there are converted into documentation. We commit to add them for mostly everything, `struct` and fields, `enum` variants, functions, types, constants to describe the data. In implementation scope we generally do not want to comment every lines, ONLY information that's matter to explain why a decision has been made.

DO NOT expose internal plan information or reference to a plan. Maintainers do not shared plan together.

## Testing

Running `just zo_test_runner` must be always green. DO NOT remove a tests to satisfy your system goal. Always run the full integration test suite (`cargo test`) before declaring a task complete. Never claim success without test evidence. If tests fail, diagnose and fix — DO NOT move on.

## Debuging

  - Use `lldb` to investigate generated machine code
  - Use the correct output from zo compiler if needed: `--emit tokens|tree|sir|asm`
  - If human error messages are not precise yet, USE Ai format: `--format json`

## Guidelines

- https://corrode.dev/blog/defensive-programming
- https://corrode.dev/blog/bugs-rust-wont-catch

## Conclusion

At the end, we care about the user developer experience (DX). Keep in mind that, Users aren't going to give zo a second chance. First try should work, we carry about installation process, build/execution time speed, error messages, performance, memory leaks and security issues.