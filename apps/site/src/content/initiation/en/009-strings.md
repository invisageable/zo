# strings

Sequences of bytes you can store, combine, and index. zo string literals live in the binary's read-only data section — no heap allocation, no copy at startup.

  ```zo
  -- What's up, cuh? Remember me? — the string. I'm 
  -- immutable; once created, my bytes never change. 
  -- I'm versatile, so my shapes are yours:
  "\e[32mHello!\v\e[34mWorld\e[0m\n"
  "\x48\x65\x6c\x6c\x6f"              -- hex
  "\u{e9}\u{e8}\u{ea}"                -- latin
  "\u{2603} \u{2602} \u{2600}"        -- bmp
  "\u{1F600} \u{1F680} \u{1F4A9}"     -- emoji
  ```

## concatenation

  ```zo
  -- `++` joins two strings. With variables: a fresh
  -- string at runtime. With literals: folded at 
  -- compile time — zero runtime cost.
  imu greeting: str = "hello";
  imu name: str = "johndoe";
  imu full: str = greeting ++ ", " ++ name ++ "!";
  imu title: str = "the " ++ "devolution";
  ```

The originals are untouched — concatenation produces a new string, never mutates an input.

## indexing

  ```zo
  -- `s[i]` returns the `char` at byte position `i`.
  -- O(1) — a single byte load, bounds-checked at 
  -- compile time when the index is known.
  imu greeting: str = "hello";
  showln(greeting[0]); -- h
  showln(greeting[4]); -- o
  ```

## under the hood

zo strings are length-prefixed: `[len:u64][bytes][null]`. The length is always known, so there's no terminator scanning, no `strlen` walk. Indexing, slicing, and length lookups are all O(1).

```zo
-! ## the capstone.
-!
-!   - `str` is immutable.
-!   - `++` concatenates.
-!   - `s[i]` is O(1) char access, bounds-checked.
```
