/// Parsed Excel data ready for rendering.
#[derive(Debug, Clone, Default)]
pub struct XlsData {
  /// List of sheet names in the workbook.
  pub sheet_names: Vec<String>,
  /// Currently selected sheet index.
  pub selected_sheet: usize,
  /// Column headers (first row or auto-generated).
  pub headers: Vec<String>,
  /// Data rows (limited to MAX_DISPLAY_ROWS).
  pub rows: Vec<Vec<String>>,
  /// Total row count before limiting.
  pub total_rows: usize,
  /// Parse error message if file is malformed.
  pub parse_error: Option<String>,
}

impl XlsData {
  /// Creates an empty Excel data structure with an error message.
  pub fn error(message: String) -> Self {
    Self {
      sheet_names: Vec::new(),
      selected_sheet: 0,
      headers: Vec::new(),
      rows: Vec::new(),
      total_rows: 0,
      parse_error: Some(message),
    }
  }

  /// Checks if there was a parse error.
  pub fn has_error(&self) -> bool {
    self.parse_error.is_some()
  }

  /// Returns true if the table was truncated (more rows than displayed).
  pub fn is_truncated(&self) -> bool {
    self.total_rows > self.rows.len()
  }

  /// Returns true if there are multiple sheets.
  pub fn has_multiple_sheets(&self) -> bool {
    self.sheet_names.len() > 1
  }
}
