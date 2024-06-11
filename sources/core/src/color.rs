//! ...

#[inline]
pub(crate) const fn error() -> ariadne::Color {
  ariadne::Color::Rgb(232, 65, 24)
}

#[inline]
pub(crate) const fn help() -> ariadne::Color {
  ariadne::Color::Rgb(246, 229, 141)
}

#[inline]
pub(crate) const fn hint() -> ariadne::Color {
  ariadne::Color::Rgb(56, 173, 169)
}

#[inline]
pub(crate) const fn note() -> ariadne::Color {
  ariadne::Color::Rgb(15, 188, 249)
}

#[inline]
pub(crate) const fn title() -> ariadne::Color {
  ariadne::Color::Rgb(112, 161, 255)
}

#[inline]
pub(crate) const fn warning() -> ariadne::Color {
  ariadne::Color::Rgb(246, 229, 141)
}
