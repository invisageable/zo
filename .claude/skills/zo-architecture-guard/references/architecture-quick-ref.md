# zo Architecture Quick Reference

## Pipeline

```
Source Text -> Tokens -> Tree -> SIR -> Machine Code
                 |                |
                 |                +-> Copilord (async, read-only)
                 |
            Parse Wave -> Lowering Wave -> Codegen Wave
```

## Execution-Based Compilation

- Tree (`zo-tree`) is the parse tree. No types, no analysis. Fast to build.
- SIR is produced by **executing** Tree, not walking it.
- Type checking happens **during** SIR building as a side effect.
- Single pass. No re-walks. No annotation tables.
- Types flow through a stack machine during execution.

## Performance Targets

| Phase | Target |
|-------|--------|
| Tokenize + Parse | 10M LoC/s |
| Semantic Analysis (Tree -> SIR) | 5M LoC/s |
| Codegen (SIR -> machine code) | 5M LoC/s |

## Parallelism Model

- MPSC Scheduler orchestrates, rayon executes.
- Work divided into Waves. Wave N+1 waits for Wave N.
- All shared data: `Send + Sync`.

## Memory Rules

- Stack allocation preferred.
- Arena allocation for IR nodes.
- Zero-allocation in tokenizer/parser hot paths.
- No heap allocation without justification.

## Forbidden Patterns

- Incremental compilation
- Complex caching schemes
- Bidirectional type systems
- Copilord -> AOT data flow
- Blocking I/O in AOT path
- Tree walking for semantic analysis
- Multiple passes over Tree
