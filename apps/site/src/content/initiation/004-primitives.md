# primitives

## numbers

Programming is just moving data around. In zo, data comes in a few basic
flavors called "primitives".

You don't need to memorize all of these right now. Just know they exist. The
most important thing: zo is smart. It "infers" types so you don't have to be
a keyboard typist just to declare a number.

> All snippets in this guide run inside `fun main()`. Wrap them when you copy-paste.

### integers

  ```zo
  -! ## the integer family.
  -!
  -!   signed:    s8, s16, s32 (int), s64
  -!   unsigned:  u8, u16, u32 (uint), u64

  -- "Hi, I'm `int` — a signed 32-bit integer, the default for any bare
  -- number you write. I scale up to s64 if you need more room, and zo
  -- supports big numbers natively — `600851475143` just works."
  showln(42);
  showln(600851475143);
  ```

### floats

  ```zo
  -! ## the float family.
  -!
  -!   f32: 32-bit (for GPUs/Games)
  -!   f64: 64-bit (float/default)

  -- "Hey, I'm `float` — a 64-bit double. Add a `.0` and you get me.
  -- I also speak scientific: `1.0e10`, `2.5e-3` — same family, different
  -- notation."
  showln(14.0);
  showln(3.14159);
  showln(1.0e10);
  showln(2.5e-3);
  ```

### bases

  ```zo
  -- "We're integers in disguise — same value, different notation. The
  -- prefix is just how *you* write us; `showln` always prints decimal."
  showln(0b11110000); -- binary       → 240
  showln(0o77);       -- octal        → 63
  showln(0xff);       -- hexadecimal  → 255
  ```

### parse modifiers

  ```zo
  -- "Same as the prefix forms, just inline shorthand."
  showln(b#30); -- binary       → 24
  showln(o#75); -- octal        → 61
  showln(x#76); -- hexadecimal  → 118
  ```

## strings

  ```zo
  -- "Hi, I'm `str` — a string literal. I live in the binary's read-only
  -- data section, so I cost nothing at runtime."
  showln("JOiN THE DEVOLUTiON.");
  ```

## chars

  ```zo
  -- "And me, I'm `char` — a single Unicode scalar wrapped in single quotes."
  showln('z');
  ```

## bytes

  ```zo
  -- "Call me `byte` — raw byte value, written with backticks. No Unicode,
  -- just the eight bits."
  showln(`z`);
  ```

## booleans

  ```zo
  -- "Hey, we're `bool` — only `true` and `false`. No 'truthy' or 'falsy'
  -- mind games here."
  showln(true);
  showln(false);
  ```
