# hello

Let's spin up your first interactive instance. We trigger text printing operations via an internal optimized wrapper.

  ```zo
  -! Yo! Let's start with a simple program.
  -! In this lesson, we learn how to print a message.

  fun main() {
    -- We call our buddy `showln`.
    -- It tells the compiler to display the value
    -- passed as argument with a newline at the end.
    showln("hello, hacker");
  }

  -! ## the capstone.
  -!
  -!   - `showln` is an internal compiler builtin. No
  -!     import or namespace matching required.
  ```
