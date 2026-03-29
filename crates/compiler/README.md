# crates — compiler.

> *the zo ecosystem crates.*

## about.

@SEE: [zo](../compiler/zo)

## commands.

**-programming-mode**

`build` YOUR PROGRAM AS AN EXECUTABLE:

  ```sh
  zo -- build hello.zo -o hello
  ```

> it will creates an executable named `hello` in the current folder.

**-templating-mode**

  `run` YOUR PROGRAM iN A NATiVE WiNDOW<sup>`winit`</sup> OR A LiGHTWEiGHT WEBViEW<sup>`wry`</sup> DEPENDiNG OF THE TARGET.

  ```sh
  zo -- run zsx-hello.zo
  ```

> this command is only for `native` app, if you want to build for `web` you should add the `--web` flag.

## dev.

FOR AN iNTRODUCTiON, [HERE](./zo-notes/public/guidelines/02-install.md) iS WHERE iT STARTS.

## release.

THE zo ECOSYSTEM iNCLUDES zo AND fret, TO RELEASE A NEW VERSiON, WE DO THE FOLLOWiNG.

  1. BUMP ALL VERSiONS:

  ```sh
  just bump_zo patch      # 0.1.0 → 0.1.1
  just bump_fret patch    # same
  ```
  
  2. VERiFY THE BUMP CORRECTNESS:

  ```sh
  just list_versions      # check versions
  just pre-commit         # all tests pass
  ```

  3. THEN COMMiT AND TAG:

  ```sh
  git add -A
  git commit -m "ops(zo): release: `0.1.1`"
  ```

  > here is our git naming-convention [guidelines](./zo-notes/public/guidelines/01-introduction.md#git-naming-convention).

  4. FiNALLY, CREATE THE TAG AND PUSH EVERYTHiNG:

  ```sh
  just release 0.1.1
  ```
