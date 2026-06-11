# install.

> *This is the beginning of the devolution.*

## get started.

  1. RUN THE iNSTALLATiON SCRiPT:

  **macos**

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

iT DOWNLOADS AND EXTRACTS THE zo COMPiLER iNTO `bin/zo` FOLDER, SETS THE ENViRONMENT WiTH THE `PATH` TO RETRiEVE THE zo BiNARY.

  2. THEN RUN `zo --version` — SUCCESSFULLY iT RETURNS:

  ```
  zo x.x.x
  ```

> *The version will depends of the latest release artifact.*

  3. DONE! TO KNOW HOW YOU CAN COMPiLE A zo PROGRAM:

  - @SEE: [@build-run-and-repl](./03-build-run-and-repl.md)

## dev.

WE ARE CONViNCE THAT'S EVERYTHiNG HAS TO BE DOCUMENTED, WE ARE NOT FULLY DOCUMENTED BUT THiNGS ARE GOiNG WELL. EACH CRATE CONTAiNS A README WHiCH PROViDES DETAiLS ABOUT ARCHiTECTURE CHOiCES, BENCHMARK RESULTS AND iMPLEMENTATiON EXPLANATiON. JUMP TO A SPECiFiC README'S CRATE TO CLEARLY UNDERSTAND HOW iT WORKS.

**-quick-and-run**

iNSTALL `rust`, `cargo` and `just` ON YOUR MACHiNE.

  - @SEE [`rust & cargo — install`](https://rust-lang.org/tools/install)
  - @SEE [`just — packages`](https://just.systems/man/en/packages.html)

SETUP THE DEV ENViRONMENT WiTH THE FOLLOWING RECiPE:

  ```sh
  just setup
  ```

UNFORTUNALY SOME TOOLS NEEDS TO BE iNSTALL MANUALLY:

  - @SEE [`nextest — pre-build binaries`](https://nexte.st/docs/installation/pre-built-binaries)

**-optional-tools**

  - @SEE — [`zo-vscode plugin`](../../../zo-vscode)

---

[prev](./00-prologue.md) — [next](./02-build-run-and-repl.md)
