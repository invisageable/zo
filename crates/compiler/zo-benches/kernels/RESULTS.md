# Runtime Kernels — Codegen Quality

These measure **runtime** (how fast the emitted machine code executes), not compile time like `../benches/`. Each kernel isolates one axis of codegen quality, compared against the same C compiled at `-O0` and `-O2`.

## macOS (Apple Silicon M-series)

Best-of-5 warm wall time (ms), `arm64-apple-darwin`:

| kernel | what it stresses                  | zo      | clang -O0 | clang -O2 | zo vs -O2 |
| :----- | :-------------------------------- | ------: | --------: | --------: | --------: |
| `mix`  | register pressure (9 live locals) | **105** |       563 |       103 | **1.02×** |
| `prng` | latency-bound LCG step            |   206   |       204 |       153 |     1.35× |
| `fib`  | call + frame overhead             |   139   |       173 |       101 |     1.38× |

## Notes

- **`mix` is the headline.** Before scalar-local register promotion (mem2reg), zo ran it in **563ms** — tied with unoptimized C, because all 9 `mut` locals round-tripped through the stack every iteration. Promoting them into callee-saved registers (x19–x27) dropped it to **105ms — matched `-O2`**, a 5.4× win. The loop body is now pure `add`/`mov`; the per-iteration `ldr`/`str` are gone.
- **`prng` / `fib` are latency- and call-bound**, so the out-of-order CPU hides most memory traffic — zo ties or beats `-O0` and sits ~1.35× off `-O2`. The remaining gap is `-O2`-level instruction selection and inlining, not register residency.
- These are the regression guard for codegen-quality work: a change that reintroduces stack traffic shows up immediately in `mix`.

## Reproduce

```sh
ZO=target/release/zo
for k in mix prng fib; do
  $ZO build crates/compiler/zo-benches/kernels/$k.zo -o /tmp/${k}_zo
  clang --target=arm64-apple-darwin -O0 crates/compiler/zo-benches/kernels/$k.c -o /tmp/${k}_c0
  clang --target=arm64-apple-darwin -O2 crates/compiler/zo-benches/kernels/$k.c -o /tmp/${k}_c2
done
# time each: best-of-5 warm runs, stdout to /dev/null
```
