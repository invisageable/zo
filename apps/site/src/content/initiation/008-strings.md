# strings

Sequences of bytes you can store, combine, and index. zo string literals live in the binary's read-only data section — no heap allocation, no copy at startup.

  ```zo
  -- "Hi, I'm `str` — a string. I'm immutable; once created, my bytes
  -- never change. Reach for me anywhere you need text."
  imu greeting: str = "hello";
  imu name: str = "johndoe";
  showln(greeting);
  ```

## concatenation

  ```zo
  -- `++` joins two strings. With variables: a fresh string at runtime.
  -- With literals: folded at compile time — zero runtime cost.
  imu greeting: str = "hello";
  imu name: str = "johndoe";
  imu full: str = greeting ++ ", " ++ name ++ "!";
  showln(full); -- hello, johndoe!

  imu title: str = "the " ++ "devolution"; -- folded at compile time
  showln(title); -- the devolution
  ```

The originals are untouched — concatenation produces a new string, never mutates an input.

## indexing

  ```zo
  -- `s[i]` returns the `char` at byte position `i`. O(1) — a single
  -- byte load, bounds-checked at compile time when the index is known.
  imu greeting: str = "hello";
  showln(greeting[0]); -- h
  showln(greeting[4]); -- o
  ```

## under the hood

zo strings are length-prefixed: `[len:u64][bytes][null]`. The length is always known, so there's no terminator scanning, no `strlen` walk. Indexing, slicing, and length lookups are all O(1).

## try this

- Concat three variables into one sentence.
- Try `"hello" ++ 42` — the compiler stops you on type mismatch.
- Build a multi-line message with `++` and `"\n"`.

```zo
-! ## the capstone.
-!
-!   - `str` is immutable; operations return new strings.
-!   - `++` concatenates; literal-only forms fold at compile time.
-!   - `s[i]` is O(1) char access, bounds-checked.
```
