# frequently asked questions (faq)

## basics

**What languages inspired zo?**

Above all Rust-prehistory — Graydon Hoare's original Rust, the language zo grew out of. Beyond it, zo draws on Carbon, Jai, Erlang, Go, Cyclone, Imba, E4X, and Elm. The [specification](/spec) names each lineage and what it lent.

**I'm new to zo. Where should I start?**

The quick way is to do the [initiation](https://zo.compilords.house/initiation) to get a full understanding of specific concept.

**Does zo have a playground?**

Yes — it ships inside codelord, the zo editor. [Download codelord](#) to try zo there — coming soon.

**Where can I get support?**

Reach the team on [Discord](https://discord.gg/JaNc4Nk5xw) — look in the zo category. For bugs and feature requests, open a [GitHub issue](https://github.com/invisageable/zo/issues).

**How do I get syntax highlighting for `.zo` files?**

A [VS Code extension](https://github.com/invisageable/zo/tree/main/crates/compiler/zo-vscode) is available.

## usage

**Can I use zo for servers?**

Yes. The standard library has a non-blocking TCP layer — `TcpListener` to bind and accept, `TcpStream` to read and write — and an HTTP/1.1 stack on top: `HttpServer::new(addr).listen_and_serve(handler)`. Each connection runs as a lightweight task, so one server handles many at once without blocking.

## comptime

**How does zo achieve sub-second compilation speeds?**

zo's speed comes from a short pipeline and owning the whole stack. It runs Tree directly to produce SIR, folding semantic analysis and lowering into one pass rather than several — without skipping any of the safety work a conventional compiler does.

Every stage is hand-written for control over speed, and zo emits the binary itself: its own assembler and Mach-O linker produce a native, code-signed executable directly. So an external system linker — `ld` or `lld`, with its startup-and-link cost — never enters the loop.

In the [benchmarks](https://github.com/invisageable/zo/tree/main/crates/compiler/zo-benches), zo compiles faster than clang, gleam, go, odin, and rustc.

## memory management

**If there is no garbage collector (GC), and no borrow checker, what is the memory management model?**

zo gives every value a single owner. When the owner leaves scope, the compiler inserts its destructor, so a `Vec`, `HashMap`, `String`, or any value that owns a resource is freed at the closing brace. There is no garbage collector and no reference counting; the frees are placed at compile time and cost nothing at runtime.

Ownership is affine: a value has one owner, and using it after it moves is a compile-time error. That is the check zo runs in place of a borrow checker.

To release a resource early, call `.free()`. It consumes the value, so the automatic drop steps aside and nothing is freed twice. One case still needs it: a resource created inside a loop body. Function and block scopes drop automatically today; loop bodies will follow.

## templating

**How does zo compile a single Ui codebase to native widgets?**

The compiler's parser turns zsx (zo Syntax Extension) into a flat, platform-agnostic stream of user-interface instructions — `UiCommand`s.

Each target lowers that stream to its own native views: `UIButton` on iOS, `android.widget.Button` on Android, `<button>` on the web, and `Button::new` on desktop.

Layout stays consistent across platforms: the runtime computes it with `taffy`, which implements the W3C Flexbox specification, fixing the exact geometry of every element. From a single codebase you get full native fidelity — VoiceOver accessibility, native keyboards, and native text rendering.

> *The gates are open, but this feature is still in development — unifying platforms is hard.*

**How do I compile to iOS or Android?**

Pass the `--target` flag: `zo build <pathname> --target ios`. The program must use the `#render` directive — it tells the compiler to enable the render runtime.

**How do I choose the Simulator device?**

`zo run <pathname> --target ios` picks a device automatically: the booted one if any, otherwise the newest iPhone on your machine. To run on a specific device, pass `--device` with any device name (or UDID) from your Simulator:

```sh
zo run app.zo --target ios --device "iPhone 17 Pro"
zo run app.zo --target ios --device "iPad (A16)"
zo run app.zo --target ios --device "Apple Vision Pro"
```

iPhone, iPad, and Apple Vision Pro simulators can all run the same `ios` build — visionOS runs it through its iOS app-compatibility layer. The Apple Watch runs its own platform build: use `--target watchos` and zo compiles, bundles, and launches the same source on a watch simulator (auto-selected, or named with `--device`):

```sh
zo run app.zo --target watchos
zo run app.zo --target watchos --device "Apple Watch Ultra 3 (49mm)"
```

If a device name does not match the target platform, the error lists every device on your machine able to run the app.

## comparison

**How does zo compare to Rust?**

zo compiles faster. If your feedback loop matters, that alone can make it the better fit. It also carries less friction: no borrow checker, no lifetimes, no function coloring. Concurrency follows Erlang, Go, and Swift — channels, lightweight tasks, and supervision.

At runtime, Rust is faster. We're working to bring zo's runtime close to C (clang).

  - @SEE [@zo-benchmark](https://github.com/invisageable/zo/blob/main/crates/compiler/zo-benches)
