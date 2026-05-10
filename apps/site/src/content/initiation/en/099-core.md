# core

## options

  ```zo
  imu some: Option = Option::Some("...");
  imu none: Option = Option::None;
  ```

## results

  ```zo
  imu pass: Result = Result::Pass("value");
  imu fail: Result = Result::Fail("error");
  ```

## errors

### the `?` operator

- short-circuit Result inside a Result-returning function
- desugaring: `expr?` ≡ `match expr { Pass(v) => v, Fail(e) => return Fail(e) }`

### error propagation

- chaining: `read_file(p)?.parse()?.validate()?`
- composing helpers that bubble up domain errors

### errors vs panics
- Result for *expected* failure modes (file not found, parse error)
- panics for *bugs* (invariant broken, indexing past length)
- never use Result to signal logic errors; never panic on user input

## ranges

  <!--
  not implemented yet
  ```zo
  imu r1: Range = 0..10;     -- exclusive end (0..9)
  imu r2: Range = 0..=10;    -- inclusive end (0..10)
  imu r3: Range = a..b;      -- runtime bounds
  ``` 
  -->

  ```zo
  for i := 0..5 {            -- iteration
    showln(i);               -- 0 1 2 3 4
  }

  imu slice = xs[2..5];      -- slicing
  ```

## collection types

### arrays

  ```zo
  imu xs: []int = [1, 2, 3, 4, 5];
  imu empty: []int = [];

  xs.sum();          -- 15
  empty.sum();       -- 0

  xs.contains(3);    -- true
  empty.contains(5); -- false

  xs.find(3);        -- 2
  empty.find(99);    -- -1

  xs.min_of()        -- 1
  xs.max_of()        -- 5
  empty.min_of();    -- 0
  ```

### vectors

  ```zo
  mut v: Vec<int> = Vec::new();

  v.len();      -- 0

  v.is_empty(); -- true

  v.push(10);
  v.push(20);
  v.push(30);

  v.get(0);     -- Option::Some(10)
  v.get(99);    -- Option::None

  v.set(1, 42); -- set in-bounds.
  !v.set(7, 0); -- set out-of-bounds returns false.

  v.pop(); -- Option::Some(30)

  v.remove(1); -- Option::Some(42)

  v.free();
  ```

### sets

  ```zo
  ```

### maps

  ```zo
  ```

### file system

  ```zo
  imu path: str = "/path/to/file";

  match write_file(path, "hi") {
    Result::Pass(_) => {},
    Result::Fail(_) => showln("write-err"),
  }

  match read_file(path) {
    Result::Pass(text) => showln(text),
    Result::Fail(_) => showln("read-err"),
  }

  -- directory
  ```
