# enums

  ```zo
  -- ...
  enum Foo {
    Bar,
    Oof(str), -- tuple
    Rab = 42, -- discriminant
  }

  -- construct
  imu foo: Foo = For::Bar;
  imu foo: Foo = For::Oof("What's crackin'?");
  ```
