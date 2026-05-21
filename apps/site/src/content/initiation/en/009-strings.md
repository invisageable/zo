# strings

Lengths are tracked explicitly. Length prefixed layout look like this: `[len:u64][bytes][null]`. Splice, index, and get size in `O(1)` time complexity.

  ```zo
  -- What's up, cuh? Remember me? — the string. I'm 
  -- immutable; once created, my bytes never change. 
  -- I'm versatile, so my shapes are yours:
  "\e[32mHello!\v\e[34mWorld\e[0m\n"
  "\x48\x65\x6c\x6c\x6f"          -- hex
  "\u{e9}\u{e8}\u{ea}"            -- latin
  "\u{2603} \u{2602} \u{2600}"    -- bmp
  "\u{1F600} \u{1F680} \u{1F4A9}" -- emoji esc. chars
  "🙈🙉🙊"                        -- emoji
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

  showln(greeting[0]); -- 'h' (O(1) bounds-checked access)
  ```

  ```zo
  -! ## the capstone.
  -!
  -!   - `str` is immutable.
  -!   - `++` concatenates.
  -!   - `s[i]` is O(1) char access, bounds-checked.
  ```
