# literals

## numbers

Programming is just moving data around. In zo, data comes in a few basic
flavors called "literals".

You don't need to memorize all of these right now. Just know they exist. The
most important thing: zo is smart. It "infers" types so you don't have to be
a keyboard typist just to declare a number.

> All snippets in this guide run inside `fun main()`. Wrap them when you copy-paste.

### integers

  ```zo
  -! Let me introduce the gang members.
  -!
  -! ## the integer family.
  -!
  -!   signed:    s8, s16, s32 (int), s64
  -!   unsigned:  u8, u16, u32 (uint), u64

  -- Ya, I'm the `int` chief — a signed 32-bit integer,
  -- the default for any bare number you write.
  -- I scale up to `s64` if you need more room.
  42
  -- I'm the Big Daddy Kane so I support big numbers 
  -- natively — `600851475143` just works.
  600851475143
  ```

### floats

  ```zo
  -! And that's the rival clan.
  -!
  -! ## the float family.
  -!
  -!   f32: 32-bit
  -!   f64: 64-bit (float)

  -- Heyo, I'm `float` — a 64-bit double. Just add a `.`
  -- and you get me. 
  14.0
  3.14159
  -- Yeah! But you don't speak scientific. Bro! Don't
  -- listen to this guy. I'm the smarter `1.0e10`, 
  -- `2.5e-3` — same family, different notation.
  1.0e10
  2.5e-3
  ```

### bases

  ```zo
  -! Let's go the basics.
  -! 
  -! ## The mask-on integer prefixes.
  -!
  -!   `0b`: binary
  -!   `0o`: octal
  -!   `0x`: hexadecimal

  -- Woop-woop, we're integers in disguise. Same value,
  -- different notation.
  0b11110000 -- 240
  0o77       -- 63
  0xff       -- 255
  ```

### parse modifiers

  ```zo
  -! ## The parser integer prefixes.
  -!
  -!   `b#`: binary
  -!   `o#`: octal
  -!   `x#`: hexadecimal

  -- Same as the prefix forms, just inline shorthand.
  b#30 -- 24
  o#75 -- 61
  x#76 -- 118
  ```

## booleans

  ```zo
  -! And we finished with The Fraternal twins.

  -- Wordup, we're `bool` — only `true` and `false`.
  -- No "truthy" or "falsy" mind games here."
  true
  false
  ```

## strings

  ```zo
  -- Bitch! I'm `str` — a string literal. I live in 
  -- the binary's read-only data section hood, so I
  -- cost nothing at runtime. Skuuuuuu!"
  "JOiN THE DEVOLUTiON."
  ```

## chars

  ```zo
  -- And me, I'm `char` — a single Unicode scalar 
  -- wrapped in single quotes.
  'z'
  ```

## bytes

  ```zo
  -! My man, introduce yourself!

  -- Call me `bytes` — raw byte value, written with 
  -- backticks. No Unicode, just the 8 bits."
  `z`
  ```
