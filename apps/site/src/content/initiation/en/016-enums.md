# enums

  ```zo
  -- ...
  enum Foo {
    Bar,
    Oof(str), -- tuple
    Rab = 42, -- discriminant
  }

  -- construct
  imu foo: Foo = Foo::Bar;
  imu foo: Foo = Foo::Oof("What's crackin'?");
  ```
