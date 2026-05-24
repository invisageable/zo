# Pike-Style Concurrent Prime Sieve — Results

Hardware-honest run of `concurrency_pike_sieve.zo` via `just zo_bench_sieve N`. Each `N` is the prime-count
target — the chain spawns exactly `N` filter green tasks dynamically as primes are discovered. Memory is
peak RSS sampled at 50 ms during a release-build run.

## macOS (Apple Silicon M-series, 16 KiB pages)

| N stages | Peak RSS    | Per-task | Wall (hot avg) | Source           |
| -------: | ----------: | -------: | -------------: | :--------------- |
|    1,000 |   20.19 MiB |   ~20 KiB | 170 ms | measured         |
|    5,000 |   85.57 MiB |   ~17 KiB |          5.6 s | measured         |
|   10,000 |  166.86 MiB |   ~17 KiB |         25.1 s | measured         |
|   50,000 |    ~830 MiB |   ~17 KiB |         ~10 m  | projected (16 KiB × N) |
|  100,000 |    ~1.6 GiB |   ~17 KiB |         ~40 m  | projected (16 KiB × N) |

## Linux (x86_64, 4 KiB pages) — see `vm.max_map_count` note

| N stages | Peak RSS  | Per-task | Source           |
| -------: | --------: | -------: | :--------------- |
|    1,000 |   ~5 MiB  |   ~5 KiB | projected (4 KiB × N) |
|   10,000 |  ~45 MiB  |   ~5 KiB | projected (4 KiB × N) |
|  100,000 |  ~450 MiB |   ~5 KiB | projected (4 KiB × N) |

## Notes

- Per-task floor on macOS is **16 KiB** — one hardware page committed on the first byte written to the task stack. The reservation is 8 MiB virtual, lazy-committed. The +4% over-projection observed at 10,000 stages (167 MiB vs 160 MiB predicted) is shared runtime overhead (scheduler, Selector, dyld, libc) — not per-task waste.
- Linux per-task floor is **4 KiB** for the same reason with a smaller page size — order-of-magnitude smaller physical footprint at the same `N`.
- **Linux 100k requires** raising `vm.max_map_count` above 65,530 (the default): `sudo sysctl -w vm.max_map_count=262144`. Each task stack is one `mmap` region; the default cap rejects the 65,531st `mmap`. Post-MVP, a slab-arena `stack::reserve` would collapse this to a single mapping and remove the prerequisite.
- Wall time scales quadratically — `N` filter stages routing through `O(N)` integers each. The bench is a scheduler / context-switch stress test, not a throughput target.

## Reproduce

```sh
just zo_bench_sieve 1000     # tiny
just zo_bench_sieve 10000    # macOS practical ceiling
just zo_bench_sieve 100000   # Linux only, sysctl prereq
```
