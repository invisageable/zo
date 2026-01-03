/// Represents a [`Artifact`] instance.
///
/// An artifact contains the generated binary code.
#[derive(Debug)]
pub struct Artifact {
  /// The generated binary code.
  pub code: Vec<u8>,
}
