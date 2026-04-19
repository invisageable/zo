// zo-runtime: minimal C helpers for intrinsics that don't
// map to libc symbols with fixed non-variadic signatures.
//
// Compiled inline by `cc` alongside the user's object file at
// link time — no separate archive, no build.rs. The source is
// embedded in `zo-linker` via `include_str!` and dropped into
// a temp file next to the user's `.o` on every link.
//
// Each wrapper has a distinct name and a fixed signature so
// the CLIF backend can declare it without hitting Cranelift's
// "one signature per external name" constraint (which blocks
// calling variadic `snprintf` with both int and float
// arguments from the same module).

#include <stdio.h>

// Formats an `f64` into `buf` using `%g` (shortest round-trip
// representation). Returns the number of characters that
// would have been written, excluding the trailing NUL —
// matches `snprintf`'s contract.
int zo_ftoa_f64(char *buf, unsigned long size, double val) {
  return snprintf(buf, size, "%g", val);
}
