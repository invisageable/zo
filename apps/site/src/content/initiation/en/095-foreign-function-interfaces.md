# foreign function interfaces

zo calls C-ABI libraries directly. You declare the foreign function once, tell the linker where its symbol lives, and call it like any zo function — no wrapper layer, no runtime cost.

## declaring a foreign function

A `pub ffi` declaration names a function that lives in a C library. It has no body; the symbol resolves at link time.

  ```zo
  -- A declaration ends in `;` — no body.
  pub ffi sqrt(x: f64) -> f64;

  -- A void function omits the return.
  pub ffi close_window();
  ```

zo drives the call straight from this signature: it places arguments in registers per the platform ABI, narrows or widens scalars, and passes structs by value. Adding a function costs one line.

## linking a library

A `#link` block tells the linker which dylib owns the symbols in the file. One block covers every `pub ffi` in the same pack.

  ```zo
  #link {
    macos: "@executable_path/libzo_provider_sqlite.dylib",
    linux: "@executable_path/libzo_provider_sqlite.so",
  }

  pub ffi zo_sqlite_open(path: CStr) -> int;
  pub ffi zo_sqlite_close(handle: int);
  ```

C strings cross the boundary as `CStr` (from `core::c`), never `str` — a zo `str` carries a length header that a C function would misread.

## calling a library

Foreign libraries ship as opt-in providers. Load one and call it:

  ```zo
  load core::c::*;
  load provider::sqlite::*;

  fun main() {
    imu db: int = zo_sqlite_open(CStr::new("scores.db"));
    zo_sqlite_exec(db, CStr::new("CREATE TABLE s (score int)"));
    zo_sqlite_close(db);
  }
  ```

## the binding generator

Hand-writing a `pub ffi` line per function — and keeping each one in sync with the library — is the tedious part. `zo-binder` writes those declarations for you, from one of two sources.

### from a rust library

Wrap any Rust crate in a small shim that exports plain C functions:

  ```rust
  #[unsafe(no_mangle)]
  pub extern "C" fn undo_stack_new() -> i64 { /* ... */ }
  ```

Then generate the bindings:

  ```sh
  just bind undoredo
  ```

zo-binder reads the shim's signatures and writes `provider/undoredo/undoredo.zo` — the `#link` block plus one `pub ffi` per function, ready to `load`.

### from a c header

For a C library, feed zo-binder the header's machine-readable API. raylib emits one through its `rlparser` tool:

  ```sh
  zo-binder --json raylib_api.json --lib raylib \
    --macos /opt/homebrew/lib/libraylib.dylib \
    --linux /usr/lib/x86_64-linux-gnu/libraylib.so
  ```

zo-binder maps each C type to its zo equivalent, generates the structs, renames `InitWindow` to `init_window`, and skips what it cannot map — callbacks, variadics — reporting each one.

The generated file is committed and reviewed like any other source. Nothing runs during `zo run`: you regenerate bindings deliberately, the way you would run a formatter.
