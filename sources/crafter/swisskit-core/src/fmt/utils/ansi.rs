/// Strips ANSI escape sequences from a byte slice.
///
/// Handles CSI sequences (`ESC [ ... letter`) which cover
/// colors, cursor movement, and styling. Non-CSI escapes
/// are passed through.
pub fn strip_ansi(s: &str) -> String {
  let bytes = s.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;

  while i < bytes.len() {
    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
      // Skip CSI: ESC [ params letter.
      i += 2;

      while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
        i += 1;
      }

      // Skip the terminating letter.
      if i < bytes.len() {
        i += 1;
      }
    } else {
      out.push(bytes[i]);
      i += 1;
    }
  }

  // SAFETY: input is valid UTF-8 and we only removed ASCII escape sequences —
  // no multi-byte boundaries can be broken.
  unsafe { String::from_utf8_unchecked(out) }
}

// fn strip_ansi(s: &str) -> String {
//   let mut result = String::with_capacity(s.len());
//   let mut in_escape = false;

//   for c in s.chars() {
//     if c == '\x1b' {
//       in_escape = true;
//     } else if in_escape {
//       if c.is_ascii_alphabetic() {
//         in_escape = false;
//       }
//     } else {
//       result.push(c);
//     }
//   }

//   result
// }

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_no_escapes() {
    assert_eq!(strip_ansi("hello world"), "hello world");
  }

  #[test]
  fn test_color_codes() {
    assert_eq!(strip_ansi("\x1b[31mError:\x1b[0m bad"), "Error: bad");
  }

  #[test]
  fn test_256_color() {
    assert_eq!(strip_ansi("\x1b[38;5;246m│\x1b[0m"), "│");
  }

  #[test]
  fn test_empty() {
    assert_eq!(strip_ansi(""), "");
  }

  #[test]
  fn test_only_escapes() {
    assert_eq!(strip_ansi("\x1b[31m\x1b[0m"), "");
  }
}
