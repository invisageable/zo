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
  load std::math::pow;
  load std::math::(pow, abs);
  ```
