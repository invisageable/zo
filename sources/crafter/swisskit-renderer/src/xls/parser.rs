//! XLS/XLSX parser for preview rendering.
//!
//! Parses Excel files into structured data for table rendering.
//! Supports both legacy .xls and modern .xlsx formats via calamine.

use crate::xls::data::XlsData;

use calamine::{Data, Reader, open_workbook_auto};

use std::io::{Read, Seek};
use std::path::Path;

/// Maximum rows to display in table view (performance limit).
const MAX_DISPLAY_ROWS: usize = 1000;

/// Parses an Excel file from a file path.
pub fn parse_xls_file(path: &Path) -> XlsData {
  match open_workbook_auto(path) {
    Ok(mut workbook) => parse_workbook(&mut workbook),
    Err(e) => XlsData::error(format!("Failed to open Excel file: {e}")),
  }
}

/// Parses an Excel file from bytes with format hint.
pub fn parse_xls_bytes(bytes: &[u8], extension: &str) -> XlsData {
  let cursor = std::io::Cursor::new(bytes);

  match extension.to_lowercase().as_str() {
    "xlsx" | "xlsm" | "xlsb" => match calamine::Xlsx::<_>::new(cursor) {
      Ok(mut workbook) => parse_workbook(&mut workbook),
      Err(e) => XlsData::error(format!("Failed to parse XLSX: {e}")),
    },
    "xls" => match calamine::Xls::<_>::new(cursor) {
      Ok(mut workbook) => parse_workbook(&mut workbook),
      Err(e) => XlsData::error(format!("Failed to parse XLS: {e}")),
    },
    "ods" => match calamine::Ods::<_>::new(cursor) {
      Ok(mut workbook) => parse_workbook(&mut workbook),
      Err(e) => XlsData::error(format!("Failed to parse ODS: {e}")),
    },
    _ => XlsData::error(format!("Unsupported spreadsheet format: {extension}")),
  }
}

/// Parses a workbook into XlsData using the Reader trait.
fn parse_workbook<R: Reader<RS>, RS: Read + Seek>(workbook: &mut R) -> XlsData {
  let sheet_names = workbook.sheet_names().to_vec();

  if sheet_names.is_empty() {
    return XlsData::error("No sheets found in workbook".to_string());
  }

  // Parse first sheet by default
  parse_sheet_internal(workbook, &sheet_names, 0)
}

/// Parses a specific sheet from a workbook.
pub fn parse_sheet_at(path: &Path, sheet_index: usize) -> XlsData {
  match open_workbook_auto(path) {
    Ok(mut workbook) => {
      let sheet_names = workbook.sheet_names().to_vec();
      parse_sheet_internal(&mut workbook, &sheet_names, sheet_index)
    }
    Err(e) => XlsData::error(format!("Failed to open Excel file: {e}")),
  }
}

/// Internal helper to parse a specific sheet.
fn parse_sheet_internal<R: Reader<RS>, RS: Read + Seek>(
  workbook: &mut R,
  sheet_names: &[String],
  sheet_index: usize,
) -> XlsData {
  let sheet_name = match sheet_names.get(sheet_index) {
    Some(name) => name.clone(),
    None => return XlsData::error("Sheet index out of bounds".to_string()),
  };

  let range = match workbook.worksheet_range(&sheet_name) {
    Ok(r) => r,
    Err(e) => {
      return XlsData::error(format!(
        "Failed to read sheet '{sheet_name}': {e:?}",
      ));
    }
  };

  if range.is_empty() {
    return XlsData {
      sheet_names: sheet_names.to_vec(),
      selected_sheet: sheet_index,
      headers: vec!["Empty Sheet".to_string()],
      rows: Vec::new(),
      total_rows: 0,
      parse_error: None,
    };
  }

  let mut all_rows = Vec::new();

  for row in range.rows() {
    let row_data = row.iter().map(cell_to_string).collect::<Vec<_>>();

    all_rows.push(row_data);
  }

  if all_rows.is_empty() {
    return XlsData {
      sheet_names: sheet_names.to_vec(),
      selected_sheet: sheet_index,
      headers: vec!["Empty Sheet".to_string()],
      rows: Vec::new(),
      total_rows: 0,
      parse_error: None,
    };
  }

  // Determine column count (max columns in any row)
  let column_count = all_rows.iter().map(|row| row.len()).max().unwrap_or(0);

  if column_count == 0 {
    return XlsData::error("No columns detected in sheet".to_string());
  }

  // Auto-detect headers: check if first row looks like headers
  let first_row_is_headers = if let Some(first_row) = all_rows.first() {
    first_row.iter().all(|cell| {
      // Consider it a header if it's not a pure number and not empty
      cell.parse::<f64>().is_err() && !cell.is_empty()
    })
  } else {
    false
  };

  let (headers, data_rows) = if first_row_is_headers && all_rows.len() > 1 {
    let mut headers = all_rows[0].clone();
    // Pad headers if needed
    while headers.len() < column_count {
      headers.push(format!("Column {}", headers.len() + 1));
    }

    (headers, &all_rows[1..])
  } else {
    let headers = (1..=column_count)
      .map(|i| format!("Column {i}"))
      .collect::<Vec<_>>();

    (headers, all_rows.as_slice())
  };

  let total_rows = data_rows.len();

  // Limit rows for performance
  let display_rows = data_rows
    .iter()
    .take(MAX_DISPLAY_ROWS)
    .map(|row| {
      let mut padded_row = row.clone();
      // Pad rows to match column count
      while padded_row.len() < column_count {
        padded_row.push(String::new());
      }
      // Truncate if row is too long
      padded_row.truncate(column_count);
      padded_row
    })
    .collect::<Vec<_>>();

  XlsData {
    sheet_names: sheet_names.to_vec(),
    selected_sheet: sheet_index,
    headers,
    rows: display_rows,
    total_rows,
    parse_error: None,
  }
}

/// Converts a calamine Data cell to a string.
fn cell_to_string(cell: &Data) -> String {
  match cell {
    Data::Empty => String::new(),
    Data::String(s) => s.clone(),
    Data::Float(f) => {
      // Format floats nicely (no trailing zeros for integers)
      if f.fract() == 0.0 {
        format!("{f:.0}")
      } else {
        format!("{f}")
      }
    }
    Data::Int(i) => i.to_string(),
    Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
    Data::Error(e) => format!("#ERROR: {e:?}"),
    // calamine ExcelDateTime can be converted to string
    Data::DateTime(dt) => format!("{dt}"),
    Data::DateTimeIso(s) => s.clone(),
    Data::DurationIso(s) => s.clone(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cell_to_string_empty() {
    assert_eq!(cell_to_string(&Data::Empty), "");
  }

  #[test]
  fn test_cell_to_string_string() {
    assert_eq!(cell_to_string(&Data::String("Hello".to_string())), "Hello");
  }

  #[test]
  fn test_cell_to_string_float() {
    assert_eq!(cell_to_string(&Data::Float(42.0)), "42");
    assert_eq!(cell_to_string(&Data::Float(3.14)), "3.14");
  }

  #[test]
  fn test_cell_to_string_int() {
    assert_eq!(cell_to_string(&Data::Int(123)), "123");
  }

  #[test]
  fn test_cell_to_string_bool() {
    assert_eq!(cell_to_string(&Data::Bool(true)), "TRUE");
    assert_eq!(cell_to_string(&Data::Bool(false)), "FALSE");
  }

  #[test]
  fn test_xls_data_error() {
    let data = XlsData::error("Test error".to_string());
    assert!(data.has_error());
    assert_eq!(data.parse_error, Some("Test error".to_string()));
  }
}
