# fret — types.

> *The collection of shared types of fret.*

## about.

CORE DATA STRUCTURES FOR THE fret BUiLD SYSTEM. FLAT, CACHE-FRiENDLY LAYOUTS FOLLOWiNG DATA-ORiENTED DESiGN.

## types.

  - `BuildContext` — *central build state flowing through all pipeline stages.*
  - `ProjectConfig` — *parsed `fret.oz` configuration*
  - `Version` — *semantic version (`major.minor.patch`).*
  - `Target` — *native compilation targets.*
  - `BuildMode` — *debug AND release.*
  - `CompilerFlags` — *flags passed to zo-compiler.*
  - `Stage` — *trait implemented by each pipeline stage.*
  - `StageError` — *error types for pipeline execution.*
