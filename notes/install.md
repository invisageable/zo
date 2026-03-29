# install.

> **

## about.

LET'S START! 

ON LiNUX AND MACOS SYSTEMS, YOU MUST RUN:

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/install.sh | sh
  ```

## dev.

FiRST, YOU NEED TO HAVE RUST AND CARGO iNSTALL iNTO YOUR MACHiNE. @SEE [`rust — install`](https://rust-lang.org/tools/install)

TO SETUP THE DEV ENViRONMENT iN YOUR MACHiNE, RUN THE FOLLOWING COMMAND:

  ```sh
  just setup
  ```

iT iNSTALLS:

- `typos` — *it checks typos in the whole codebase.*

UNFORTUNALY SOME TOOLS NEEDS TO BE iNSTALL MANUALLY DEPENDiNG OF YOUR OS:

- `nextest` — *@SEE [`nextest — pre-build binaries`](https://nexte.st/docs/installation/pre-built-binaries).*