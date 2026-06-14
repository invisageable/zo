# literals

## numbers

Programming boils down to memory allocation layout and data mutations. Data arrives in primary primitives called literals. You do not need to memorize these constraints instantly, but you must respect their sizes.

> *All contextual code snippets assume code is running inside an active `fun main()` execution block.*

### integers

  ```zo
  -! Let me introduce the gang members.
  -!
  -! ## the integer family.
  -!
  -!   signed:    s8, s16, s32 (int), s64
  -!   unsigned:  u8, u16, u32 (uint), u64

  -- Ya, I'm the `int` chief — a signed 32-bit integer
  -- by default for any bare number you write.
  -- I scale up to `s64` if you need more room.
  42
  -- I support large values natively — `600851475143` 
  -- allocates down without complex object types.
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

  -- Heyo, I'm `float` — a 64-bit double. Just add a 
  -- `.` and you get me. 
  14.0
  3.14159
  -- I support scientific notation natively. No
  -- overhead, just quick compilation values.
  1.0e10
  2.5e-3
  ```

### bases

  ```zo
  -! Mask-on integer prefixes change internal notation
  -! views.

  0b11110000 -- Binary notation evaluates to 240
  0o77       -- Octal notation evaluates to 63
  0xff       -- Hexadecimal notation evaluates to 255
  ```

### parse modifiers

  ```zo
  -! A `#` prefix sets the display base. The digits stay
  -! decimal; only how the value prints changes.

  b#30 -- value 30, shown in binary
  o#75 -- value 75, shown in octal
  x#76 -- value 76, shown in hexadecimal
  ```

## booleans

  ```zo
  -- Wordup, we're `bool` — only `true` and `false`.
  -- No "truthy" or "falsy" mind games here."
  true
  false
  ```

## strings

  ```zo
  -- Look out! I'm `str` — a string literal. I live in 
  -- the binary's read-only data section hood, so I
  -- cost nothing at runtime. Skuuuuuu!"
  "JOiN THE DEVOLUTiON."
  ```

## chars

  ```zo
  -- Call me `char` — a single Unicode scalar wrapped
  -- in single quotes.
  'z'
  ```

## bytes

  ```zo
  -- Call me `bytes` — a multi-byte buffer wrapped in
  -- backticks. Same layout as `str`, but without the
  -- UTF-8 safety validation promise. Every raw byte 
  -- is preserved.
  `hello`
  `¥orld`
  ```
