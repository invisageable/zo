/// The diagnostic color for an `error` message.
#[inline(always)]
pub(crate) const fn error() -> ariadne::Color {
  ariadne::Color::Rgb(232, 65, 24)
}

/// The diagnostic color for a `warning` message.
#[inline(always)]
pub(crate) const fn warning() -> ariadne::Color {
  ariadne::Color::Rgb(246, 229, 141)
}

/// The diagnostic color for a `warning` message.
#[inline(always)]
pub(crate) const fn advice() -> ariadne::Color {
  ariadne::Color::Rgb(124, 0, 254)
}

/// The diagnostic color for an `help` message.
#[inline(always)]
pub(crate) const fn help() -> ariadne::Color {
  ariadne::Color::Rgb(246, 229, 141)
}

/// The diagnostic color for an `hint` message.
#[inline(always)]
pub(crate) const fn hint() -> ariadne::Color {
  ariadne::Color::Rgb(56, 173, 169)
}

/// The diagnostic color for a `note` message.
#[inline(always)]
pub(crate) const fn note() -> ariadne::Color {
  ariadne::Color::Rgb(15, 188, 249)
}

/// The diagnostic color for a title.
#[inline(always)]
pub(crate) const fn title() -> ariadne::Color {
  ariadne::Color::Rgb(112, 161, 255)
}
