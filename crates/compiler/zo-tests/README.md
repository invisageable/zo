# zo — tests.

## about.

ALL TESTS ARE SPLiT BY CONCEPT, PROGRAMS THAT USE THE `build` VS `run` COMMANDS GOES iNTO A SEPARATE FOLDER. WE ARE NOT ALREADY CONViNCE ON HOW WE ORGANIZED iT BUT FOR NOW iT'S FiNE; YOU CAN NAViGATE AND SEE ALL PROGRAMS. EACH CATEGORY CONTAiNS TWO FOLDERS, ONE FOR `programming`, THE SECOND FOR `templating` MODE.    

SOME OF THESE TESTS ARE BEiNG PORTED FROM GRAYDON HOARE'S ORiGiNAL RUST COMPiLER TEST SUiTE — [rust-prehistory/src/test](https://github.com/graydon/rust-prehistory/tree/master/src/test).

THE GOAL iS TO PROVE zo'S COMPiLER CORRECTNESS BY PASSiNG THE SAME FOUNDATiONAL TESTS THAT VALiDATED THE VERY FiRST RUST COMPiLER. WE FOLLOW OUR MASTERS (S/O [@graydon](https://github.com/graydon)).

port: https://github.com/elm/error-message-catalog

## commands.

```bash
cargo run --bin zo -- build <input> -o <output>
```
