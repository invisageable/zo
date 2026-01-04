#[derive(Debug)]
pub enum PdfError {
  Init(String),
  Io(std::io::Error),
  Parse(String),
  Render(String),
}

impl std::fmt::Display for PdfError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PdfError::Init(e) => write!(f, "Pdfium init error: {e}"),
      PdfError::Io(e) => write!(f, "IO error: {e}"),
      PdfError::Parse(e) => write!(f, "Parse error: {e}"),
      PdfError::Render(e) => write!(f, "Render error: {e}"),
    }
  }
}

impl std::error::Error for PdfError {}

impl From<std::io::Error> for PdfError {
  fn from(e: std::io::Error) -> Self {
    PdfError::Io(e)
  }
}
