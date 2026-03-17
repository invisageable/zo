# fret — pipeline.

## about.

build pipeline for fret projects. orchestrates all stages from config parsing to compilation via zo-compiler.

## stages.

| stage | role |
| :---- | :--- |
| `LoadConfig` | reads and parses `fret.oz` |
| `CollectSources` | discovers `.zo` files recursively |
| `ResolveDependencies` | no-op in simple mode |
| `GeneratePlan` | validates and prepares compilation plan |
| `ExecutePlan` | invokes zo-compiler via `CompileStage` |

## dependencies.

integrates directly with `zo-compiler` and `zo-codegen-backend` as libraries — no subprocesses.
