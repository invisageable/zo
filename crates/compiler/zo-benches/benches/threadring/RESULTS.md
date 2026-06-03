# Threadring (benchmarksgame) — Results

Port of the `threadring` benchmark: 503 green tasks wired into a ring, a single
integer token hopping node-to-node and decremented each hop. The node that
receives `0` prints its 1-based label and calls `process::exit`. Unlike the
sieve, the task count is fixed at 503 — `N` is the number of hops, so this is a
pure context-switch / channel-rendezvous throughput test at constant memory.

Run via `just zo_bench threadring --with-runtime --argv N`. Wall times are the
hot average (first run dropped — it pays dyld + runtime-dylib load).

## macOS (Apple Silicon M-series, 16 KiB pages)

| N (hops)   | Wall (warm) | Throughput     | Peak RSS  | Source   |
| ---------: | ----------: | -------------: | --------: | :------- |
|    100,000 |     17.9 ms |  5.6M hops/s   |  ~12 MiB  | measured |
|  1,000,000 |    129.8 ms |  7.7M hops/s   |  ~12 MiB  | measured |
| 10,000,000 |    1.311 s  |  7.6M hops/s   |  ~12 MiB  | measured |

Winning label for the canonical `N = 1000`: **498** (`1 + N mod 503`).

## Notes

- Throughput plateaus at **~7.6M hops/s** once warm — each hop is one channel
  rendezvous plus a green-task context switch. The first invocation costs
  ~380 ms of fixed dyld + dylib load; it is excluded from the warm figures.
- Peak RSS is **constant at ~12 MiB** for every `N`: 503 task stacks committing
  one 16 KiB page each (~8 MiB) plus ~4 MiB of shared runtime (scheduler,
  selector, dyld, libc). Hops do not allocate, so memory does not grow with `N`.
- The ring is a real cycle, not an open chain: node 503 is handed node 1's entry
  sender, so it forwards back into node 1. The token's owner kills the looping
  siblings with `process::exit`, mirroring the reference `os.Exit`.

## Reproduce

```sh
just zo_bench threadring                                  # compile only
cargo run --release -p zo-benches -- threadring \
  --with-runtime --argv 1000000                           # timed run
```
