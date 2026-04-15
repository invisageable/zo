# crates — compiler.

> *The zo ecosystem crates.*

## about.

- @SEE: [zo](../compiler/zo)

## dev.

FOR AN iNTRODUCTiON, [HERE](./zo-notes/public/guidelines/00-prologue.md) iS WHERE iT STARTS.

### compiler pipeline.

WE HAVE ELiMiNATED ALL UNNECESSARY iNTERMEDiATE STAGES. THE FRONTEND iS A LEAN, BRUTALLY FAST DATAFLOW PiPELiNE.   

### release.

THE zo ECOSYSTEM iNCLUDES zo AND fret (not plugged yet), TO RELEASE A NEW VERSiON, WE DO THE FOLLOWiNG.

  1. BUMP ALL VERSiONS:

  ```sh
  just bump <major|minor|patch>
  ```

  2. VERiFY THE BUMP CORRECTNESS:

  ```sh
  just list_versions
  ```

> *Ensure that only zo and fret has been bumped.*

  3. THEN COMMiT:

  ```sh
  git add -A
  git commit -m "ops(zo): release `<version>`"
  ```

  > *Here is our git naming-convention [guidelines](./zo-notes/public/guidelines/01-introduction.md#git-naming-convention).*

  4. FiNALLY, CREATE THE TAG AND **PUSH** EVERYTHiNG:

  ```sh
  just release <version>
  ```

THE RELEASE WiLL RUN iN THE PiPELiNE FOR THE FOLLOWiNG TARGETS:

  - `aarch64-apple-darwin`
  - `x86_64-apple-darwin`
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`

TO UPDATE THE zo EXECUTABLE iN YOUR SYSTEM CHECK [`zo install`](./zo-notes/public/guidelines/02-install.md)
