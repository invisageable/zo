# introduction.

> *The guidelines catalog.*

## about.

...

## chapters.

- [`01-install`](./01-install.md) — 
- [`02-i`]() —

## git naming convention.

ADAPTED FROM: [conventional-commits-cheatsheet](https://gist.github.com/qoomon/5dfcdf8eec66a051ecd85625518cfd13)

  ```sh
  git commit -m"<type>(<optional scope>): <description>" \
  -m"<optional body>" \
  -m"<optional footer>"
  ```

## types.

  - CHANGES RELEVANT TO THE APi OR Ui:
    - `feat` COMMiTS THAT ADD, ADJUST OR REMOVE A NEW FEATURE TO THE APi OR Ui
    - `fix` COMMiTS THAT FiX AN APi OR Ui BUG OF A PRECEDED FEAT COMMiT
  - `refactor` COMMiTS THAT REWRiTE OR RESTRUCTURE CODE WiTHOUT ALTERiNG APi OR Ui BEHAViOR
    - `perf` COMMiTS ARE SPECiAL TYPE OF REFACTOR COMMiTS THAT SPECiFiCALLY iMPROVE PERFORMANCE
  - `style` COMMiTS THAT ADDRESS CODE STYLE (E.G., WHiTE-SPACE, FORMATTiNG, MiSSiNG SEMi-COLONS) AND DO NOT AFFECT APPLiCATiON BEHAViOR
  - `test` COMMiTS THAT ADD MiSSiNG TESTS OR CORRECT EXiSTiNG ONES
  - `docs` COMMiTS THAT EXCLUSiVELY AFFECT DOCUMENTATiON
  - `build` COMMiTS THAT AFFECT BUiLD-RELATED COMPONENTS SUCH AS BUiLD TOOLS, DEPENDENCiES, PROJECT VERSiON, ...
  - `ops` COMMiTS THAT AFFECT OPERATiONAL ASPECTS LiKE iNFRASTRUCTURE (iAC), DEPLOYMENT SCRiPTS, Ci/CD PiPELiNES, BACKUPS, MONiTORiNG, OR RECOVERY PROCEDURES, ...
  - `chore` COMMiTS THAT REPRESENT TASKS LiKE iNiTiAL COMMiT, MODiFYiNG .GiTiGNORE, ...
