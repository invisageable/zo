# install.

> **

## about.

TO **JOiN THE DEVOLUTiON** ON LiNUX, MACOS AND WiNDOWS SYSTEMS, YOU MUST RUN:

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/zo-install.sh | sh
  ```

THEN RUN `zo --help` — iT MUST RETURNS:

  ```
  The zo Programming Language

  Usage: zo <COMMAND>

  Commands:
    build  builds a program
    repl   read eval print and loop a program
    run    runs a program
    help   Print this message or the help of the given subcommand(s)

  Options:
    -h, --help  Print help
  ```

## dev.

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

  - `nextest` — *@SEE [`nextest — pre-build binaries`](https://nexte.st/docs/installation/pre-built-binaries).*

**-optional-tools**

  - `zo-vscode` — [`plugin`](..)
