# control flow

## if else

  ```zo
  -- same type inside
  if 1 == 2 {
    false
  } else if 2 == 3 {
    false;
  } else if 3 == 4 {
    false
  } else {
    true
  }
  ```

## ternary

  ```zo
  -- only as expression
  imu x: int = when true ? 1 : 2;
  ```

## pattern matching

  ```zo
  -- ...
  match 5 {
    10 => check(false),
    _ => check(true), -- wildcard
  }

  -- ...
  match "z" ++ "o" {
    "ivs" => showln(false),
    "zo" => showln(true),
    _ => showln("default"),
  }
  ```

## jumps (terminators)

  ```zo
  for i := 1..10 {
    if i == 3 { continue; }
    if i == 7 { break; }
    
    show(i);
  }

  -- 12456
  ```
