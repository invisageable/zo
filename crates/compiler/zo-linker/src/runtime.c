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
#include <stdlib.h>
#include <string.h>

// Formats an `f64` into `buf` using `%g` (shortest round-trip
// representation). Returns the number of characters that
// would have been written, excluding the trailing NUL —
// matches `snprintf`'s contract.
int zo_ftoa_f64(char *buf, unsigned long size, double val) {
  return snprintf(buf, size, "%g", val);
}

// Formats `val` into `buf` in `radix` (2/8/10/16) with
// lowercase digits, treating `val` as unsigned (matching
// `%llo` / `%llx`). Returns the length written excluding the
// trailing NUL — matches `snprintf`'s contract. The CLIF
// backend calls this for the `b#` / `o#` / `x#` display
// modifiers; printf has no binary specifier, so one helper
// covers every non-decimal base. The stored value is
// unchanged — only the printed digits differ.
int zo_itoa_radix(char *buf, unsigned long size, long long val,
                  unsigned radix) {
  if (size == 0) {
    return 0;
  }

  char tmp[65]; // 64 binary digits, unsigned view (no sign).
  unsigned long long u = (unsigned long long)val;
  int i = 0;

  if (u == 0) {
    tmp[i++] = '0';
  }

  while (u != 0) {
    unsigned d = (unsigned)(u % radix);
    tmp[i++] = (d < 10) ? (char)('0' + d) : (char)('a' + (d - 10));
    u /= radix;
  }

  int len = 0;
  while (i > 0 && (unsigned long)(len + 1) < size) {
    buf[len++] = tmp[--i];
  }
  buf[len] = '\0';

  return len;
}

// Concatenates two zo strings (`[u64 LE len, UTF-8 bytes]`
// layout from `Insn::ConstString`). Allocates a fresh buffer
// holding `[len_a + len_b, bytes_a, bytes_b]` and returns a
// pointer to it. The caller treats the result as an opaque
// string pointer — identical shape to any other zo string
// value, so it composes with `show` / `showln` / further
// `++` concatenations.
//
// Heap lifetime: the allocation is never freed. zo programs
// today don't have a GC or an `str.drop()` sink, so concat
// results leak. Acceptable for CLIF bring-up; revisit when
// a runtime lifetime story lands.
void *zo_str_concat(const void *a, const void *b) {
  unsigned long len_a = *(const unsigned long *)a;
  unsigned long len_b = *(const unsigned long *)b;
  unsigned long total = len_a + len_b;
  unsigned char *result = (unsigned char *)malloc(8 + total);

  if (!result) {
    return NULL;
  }

  *(unsigned long *)result = total;
  memcpy(result + 8, (const unsigned char *)a + 8, len_a);
  memcpy(result + 8 + len_a, (const unsigned char *)b + 8, len_b);

  return result;
}
