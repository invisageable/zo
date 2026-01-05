# fret.

> _a package manager for the zo programming language ecosystem_.

## overview.

`fret` iS DESiGNED FROM FiRST PRiNCiPLES TO ACHiEVE:

## architecture.

### pipeline design.

The `fret` pipeline is a state machine that transforms `BuildContext` through discrete stages:

```
LoadConfig → CollectSources → ResolveDependencies → GeneratePlan → ExecutePlan
```

In "Simple Mode" (no dependencies), the pipeline achieves microsecond-level performance per stage.

### key features.

- ZERO-CONFiGURATiON — _builds for simple projects_.

## configuration Format.

THE `.oz` FORMAT iS CLEAN, DECLARATiVE, AND PARSED BY A ZERO-ALLOCATiON HAND-WRiTTEN PARSER:

```oz
-- project configuration.

@pack = (
  name: "my-project",
  version: "0.1.0",
  authors: ["invisageable <you@example.com>"],
  license: "MIT OR Apache-2.0",
)
```

## usage.

```bash
# Build current directory
fret build

# Build with release optimizations
fret build --release

# Future: dependency management
fret add http@1.2.0
fret update
fret test
```
