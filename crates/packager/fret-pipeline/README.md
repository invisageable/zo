# fret — pipeline.

> *Build pipeline, all stages.*

## about.

BUiLD PiPELiNE FOR FRET PROJECTS. ORCHESTRATES ALL STAGES FROM CONFiG PARSiNG TO COMPiLATiON ViA zo-compiler.

## dev.

THiS STAGE iT'S ABOUT ORCHESTRATiON, WE GONNA NEED TO iMPROVE iT TO PROViDE HiGH-SPEED PERFORMANCE. THE PiPELiNE iNTEGRATES DiRECTLY zo-compiler AND zo-codegen-backend AS LiBRARiES — TO AVOiD NO SUBPROCESSES.

### stages.

THE ORCHESTRATiON FLOWS STAGE iN THAT ORDER:

  1. `LoadConfig` — *reads and parses `fret.oz`.*
  2. `CollectSources` — *discovers `.zo` files recursively.*
  3. `ResolveDependencies` — *no-op in simple mode.*
  4. `GeneratePlan` — *validates and prepares compilation plan.*
  5. `ExecutePlan` — *invokes zo-compiler via `CompileStage`.*
