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

  imu slice: []int = xs[2..5];   -- slicing
  ```

## collection types

### arrays

  ```zo
  imu scores: []int = [1, 2, 3, 4, 5];
  imu empty: []int = [];

  scores.sum();        -- 15
  empty.sum();         -- 0

  scores.contains(3);  -- true
  empty.contains(5);   -- false

  scores.find(3);      -- 2
  empty.find(99);      -- -1

  scores.min_of()      -- 1
  scores.max_of()      -- 5
  empty.min_of();      -- 0
  ```

### vectors

  ```zo
  mut numbers: Vec<int> = Vec::new();

  numbers.len();       -- 0

  numbers.is_empty();  -- true

  numbers.push(10);
  numbers.push(20);
  numbers.push(30);

  numbers.get(0);      -- Option::Some(10)
  numbers.get(99);     -- Option::None

  numbers.set(1, 42);  -- set in-bounds.
  !numbers.set(7, 0);  -- set out-of-bounds returns false.

  numbers.pop();       -- Option::Some(30)

  numbers.remove(1);   -- Option::Some(42)
  ```

### sets

  ```zo
  mut ids: HashSet<int> = HashSet::new();

  ids.is_empty();    -- true

  ids.insert(10);    -- true  (new key)
  ids.insert(20);
  ids.insert(10);    -- false (already present)

  ids.contains(10);  -- true
  ids.contains(99);  -- false

  ids.remove(20);    -- true
  ids.len();         -- 1
  ```

### maps

  ```zo
  mut counts: HashMap<str, int> = HashMap::new();

  counts.is_empty();         -- true

  counts.insert("a", 1);
  counts.insert("b", 2);

  counts.get("a");           -- Option::Some(1)
  counts.get("z");           -- Option::None

  counts.contains_key("b");  -- true

  counts.remove("a");        -- Option::Some(1)
  counts.len();              -- 1
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

  exists(path);                  -- true
  remove_file(path);             -- true

  imu names: []str = read_dir("/some/dir");
  ```

### directories

  ```zo
  load core::io;

  io::is_dir("/some/dir");           -- true

  io::copy("a.txt", "b.txt");        -- Result::Pass(bytes copied)

  io::remove_dir("/empty/dir");      -- Result::Pass(0)
  io::remove_dir_all("/whole/tree"); -- recursive teardown
  ```

  Each returns `Result::Fail(errno)` on failure.

### terminal

  ```zo
  load core::io;

  -- fd 0 stdin, 1 stdout, 2 stderr.
  when io::isatty(1) ? showln("interactive") : showln("piped");
  ```

## environment

  ```zo
  load core::env;

  env::current_dir();             -- "/work/dir"
  env::set_current_dir("/tmp");   -- true
  env::temp_dir();                -- the OS temp directory

  env::get("HOME");               -- "/Users/me" ("" on miss)
  env::set("KEY", "value");       -- true
  env::remove("KEY");             -- true

  imu all: []str = env::vars();   -- ["KEY=VALUE", ...]
  ```

## command-line

  ```zo
  load core::cli;
  load core::io;

  imu app: Cli = Cli::new("greet", "say hello")
    .flag("v", "verbose", "print more")
    .option("n", "name", "who to greet");

  imu parsed: Parsed = app.parse(io::args());

  parsed.has("verbose");          -- true when -v / --verbose given
  parsed.value("name");           -- Option::Some("zo")
  parsed.positionals();           -- bare arguments, in order
  ```

- `--name=zo`, `--name zo`, and `-n zo` all parse the same
- unknown dashed tokens fall through to positionals
- `app.help()` renders usage text from the registered specs
