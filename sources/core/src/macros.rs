#[macro_export]
macro_rules! impl_const_instance {
  ($($name:ident $kind:expr, )*) => {
    $(
      pub const fn $name() -> Self {
        Self { kind: $kind }
      }
    )*
  };
}

#[macro_export]
macro_rules! impl_instance {
  ($($name:ident $kind:expr, )*) => {
    $(
      pub fn $name() -> Self {
        Self { kind: $kind }
      }
    )*
  };
}
