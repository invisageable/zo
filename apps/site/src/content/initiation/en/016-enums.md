# enums

Enums manage disjoint data states safely, supporting direct discriminants along with tuple-wrapped inline data tracking blocks.

  ```zo
  -- ...
  enum Foo {
    Bar,
    Oof(str), -- Structural data tuple binding
    Rab = 42, -- Assigned explicit discriminant value
  }

  -- Instance consumption example.
  imu foo: Foo = Foo::Bar;
  imu foo: Foo = Foo::Oof("What's crackin'?");
  ```
