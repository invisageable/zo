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
  load core::math::pow_i;
  load core::math::(pow_i, abs);
  ```
