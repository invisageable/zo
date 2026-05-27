# strings

All strings adhere to valid UTF-8 formats, supporting raw character arrays, unicode hex variants, layout escapes, and structural symbols seamlessly.

  ```zo
  -- Custom strings are completely immutable. The
  -- internal data bytes never change.
  "\e[32mHello!\v\e[34mWorld\e[0m\n"
  "\x48\x65\x6c\x6c\x6f"          -- hex char
  "\u{e9}\u{e8}\u{ea}"            -- latin char
  "\u{1F600} \u{1F680} \u{1F4A9}" -- decoded emoji
  "🙈🙉🙊"                        -- raw unicode literals
  ```

## concatenation

The `++` operator acts as your layout welding torch. Merging literals fuses values immediately inside the binary compilation phase, yielding zero performance penalties at execution time.

  ```zo
  imu greeting: str = "hello";
  imu name: str = "johndoe";
  imu full: str = greeting ++ ", " ++ name ++ "!";

  -- Direct character index extraction evaluates
  -- efficiently.
  showln(greeting[0]); -- Evaluates to 'h'
  ```

  ```zo
  -! ## the capstone.
  -!
  -!   - `str` is immutable.
  -!   - `++` concatenates.
  -!   - `s[i]` for string indexing.
  ```
