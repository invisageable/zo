#[macro_export]
macro_rules! impl_const_instance {
  ($($name:ident $kind:expr, )*) => {
    $(
      #[inline]
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
      #[inline]
      pub fn $name() -> Self {
        Self { kind: $kind }
      }
    )*
  };
}
