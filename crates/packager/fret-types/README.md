# fret — types.

## about.

core data structures for the fret build system. flat, cache-friendly layouts following data-oriented design.

## types.

- `BuildContext` — central build state flowing through all pipeline stages
- `ProjectConfig` — parsed fret.oz configuration
- `Version` — semantic version (`major.minor.patch`)
- `Target` — native compilation targets
- `BuildMode` — debug / release
- `CompilerFlags` — flags passed to zo-compiler
- `Stage` — trait implemented by each pipeline stage
- `StageError` — error types for pipeline execution
