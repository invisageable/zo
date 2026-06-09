# loops

## while loop

  ```zo
  -- while:block
  mut z: int = 0;
  while z < 1_000_000_000 {
    z += 1;
  }

  -- while:line
  mut z: int = 0;
  while z < 1_000_000_000 => z += 1;
  ```

## for loop

  ```zo
  -- for:block.
  for x := 0..3 {
    showln("{x}");
  }

  -- for:block:mut.
  for mut x := 0..3 {
    x += 1
  }

  -- for:line.
  for x := 0..3 => showln("{x}");

  -- for:line:mut
  -- mutable iterator, body reassigns it.
  for mut n := 0..3 => n += 1;
  ```

## infinite loop

  ```zo
  mut x: int = 0;
  loop {
    if x == 1_000_000 {
      showln(x);
      break;
    }

    x += 1;
  }
  ```
