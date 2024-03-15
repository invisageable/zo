# zhoo-analyzer.

> *The `analyzing` phase.*

The `analyzer` performs semantic analysis on the `ast`, ensuring that the program is well-formed and complies with the language rules. It produces a fully-typed `ast`, which is then passed to the code generator module. Soon it will passed the `ast` to the optimizer module instead.    
