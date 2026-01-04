// use zo_codegen_backend::{Artifact, Platform};
// use zo_writer_macho::MachO;

// use std::fs;
// use std::os::unix::fs::PermissionsExt;
// use std::path::Path;

/// Represents a [`Writer`] instance.
pub struct Writer {}

impl Writer {
  /// Creates a new [`Writer`] instance.
  pub fn new() -> Self {
    Self {}
  }

  // pub fn write(
  //   &self,
  //   platform: Platform,
  //   output_path: &str,
  //   artifact: Artifact,
  // ) {
  //   match platform {
  //     Platform::Macos => {
  //       let macho = MachO::new();

  //       // macho.write(artifact.code);
  //     }
  //   }

  //   // if let Err(error) = self.write_executable(output_path, artifact.code)
  // {   //   // report error: Internal Error.
  //   // }
  // }

  // /// Writes binary to file and make it executable.
  // fn write_executable(
  //   &self,
  //   path: impl AsRef<Path>,
  //   binary: Vec<u8>,
  // ) -> std::io::Result<()> {
  //   fs::write(&path, binary)?;

  //   let metadata = fs::metadata(&path)?;
  //   let mut permissions = metadata.permissions();

  //   permissions.set_mode(0o755); // make executable (chmod +x).
  //   fs::set_permissions(path, permissions)?;

  //   Ok(())
  // }
}

impl Default for Writer {
  fn default() -> Self {
    Self::new()
  }
}
