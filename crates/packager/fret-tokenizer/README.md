# fret — tokenizer.

> *The lexical analysis stage.*

## about.

...

## features.

  - *on-demand tokenization (no pre-scan).*
  - *escape sequence handling in strings*
  - *line comments (`--`).*
  - *identifier/keyword discrimination (`pack`).*

## dev.

KEEP iN MiND, THAT WE TRY TO REDUCE FRiCTiON iN THE zo ECOSYSTEM — fret iS NO EXCEPTiON TO THiS RULE, MEANiNG THAT WE NEED TO QUESTiON OURSELVES: "WHAT COULD CAUSE FRiCTiON TO THE DEVELOPER?". OF COURSE THiS iS SUBJECTiVE, BUT TO US, THE RESPONSE iS SPEED. iF YOUR TOOLS ARE SLOW, YOU BECAME LESS PRODUCTiVE, YOU HAVE TO WAiT, THiS CAN BE PAiNFUL.

WE COMMIT TO PROViDE HiGH-SPEED PERFORMANCE TOKENiZER. WHAT DOES iT MEAN? iN TERM OF SPEED iT MEANS THAT OUR GOAL iS TO REACH **10M LoC/s**. THiS iS DOABLE — CHANDLER CARRUTH iS ALREADY TRYiNG TO ACHiEVE iT WiTH carbon COMPiLER ([*modernizing compiler design for carbon toolchain*](https://www.youtube.com/watch?v=ZI198eFghJk)).

WE ARE ALREADY ON THE RiGHT TRACK BUT WE NEED TO ADD BENCHMARK TO MEASURE OUR CURRENT PERFORMANCE SCORE.

**-how-it-works**

ZERO-ALLOCATiON TOKENiZER FOR THE `fret.oz` CONFiGURATiON FORMAT. WE OPERATES DiRECTLY ON BYTE SLiCES AND PRODUCES TOKENS ON-DEMAND ViA `next_token()`.
