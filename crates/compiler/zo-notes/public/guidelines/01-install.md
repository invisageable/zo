# install.

> *This is the beginning of zo. Get started.*

## about.

TO **JOiN THE DEVOLUTiON** ON LiNUX, MACOS AND WiNDOWS SYSTEMS:

  1. YOU MUST RUN:

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/zo-install.sh | sh
  ```

iT DOWNLOADS AND EXTRACTS THE zo COMPiLER iNTO `bin/zo` FOLDER, SET THE ENViRONMENT WiTH THE PATH TO RETRiEVE THE zo BiNARY.

  2. THEN RUN `zo --version` — iT MUST RETURNS:

  ```
  zo 0.3.2
  ```

> *The version will depends of the latest release artifact.*

  3. DONE! TO KNOW HOW YOU CAN COMPiLE A zo PROGRAM:

  - @SEE: [@build-run-and-repl](./03-build-run-and-repl.md)

> *zo is in work in progress, if you find bugs, please feel open an [issue](https://github.com/invisageable/zo/issues).*

## dev.

WE ARE CONViNCE THAT'S EVERYTHiNG HAS TO BE DOCUMENTED, WE ARE NOT FULLY DOCUMENTED BUT THiNGS ARE GOiNG WELL. EACH CRATE CONTAiNS A README WHiCH PROViDES DETAiLS ABOUT ARCHiTECTURE CHOiCES, BENCHMARK RESULTS, STATUS OF THE iMPLEMENTATiON AND SO ON. DO NOT HESiTATE TO JUMP TO A SPECiFiC README'S CRATE iF YOU NEED TO CLEARLY UNDERSTAND OF THiS PiECE OF SOFTWARE WORKS.

BASED ON THAT AGREEMENT, YOU SHOULD BE READY TO START. 

**-quick-and-run**

FiRST, YOU NEED TO HAVE `rust`, `cargo` and `just` iNSTALL iNTO YOUR MACHiNE.

  - @SEE [`rust & cargo — install`](https://rust-lang.org/tools/install)
  - @SEE [`just — packages`](https://just.systems/man/en/packages.html)

THEN TO SETUP THE DEV ENViRONMENT iN YOUR MACHiNE, RUN THE FOLLOWING COMMAND:

  ```sh
  just setup
  ```

iT iNSTALLS:

  - `typos` — *it checks typos in the whole codebase.*

UNFORTUNALY SOME TOOLS NEEDS TO BE iNSTALL MANUALLY DEPENDiNG OF YOUR OS:

  - @SEE [`nextest — pre-build binaries`](https://nexte.st/docs/installation/pre-built-binaries)

**-optional-tools**

  - @SEE — [`zo-vscode plugin`](../../../zo-vscode)
  - @SEE — [`fret`](../../../../packager/fret)

---

[prev](./00-prologue.md) — [next](./02-build-run-and-repl.md)
