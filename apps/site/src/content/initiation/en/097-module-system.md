# module system

## pack

  ```zo
  pack say {
    fun hello() {
      showln("hello, modular world");
    }
  }
  ```

## load

  ```zo
  load core::math::pow;
  load core::math::(pow, abs);
  ```
