//! Parse rlparser's `raylib_api.json` into a typed model.
//!
//! @note — rlparser (raylib's official header scanner) emits
//! this JSON; zo-binder consumes it instead of parsing C.

use crate::model::BindError;

use serde::Deserialize;

/// The parsed C API surface (rlparser JSON output).
#[derive(Debug, Deserialize)]
pub struct CApi {
  #[serde(default)]
  pub functions: Vec<CFunction>,
  #[serde(default)]
  pub structs: Vec<CStruct>,
  #[serde(default)]
  pub enums: Vec<CEnum>,
  #[serde(default)]
  pub aliases: Vec<CAlias>,
}

/// A C function declaration.
#[derive(Debug, Deserialize)]
pub struct CFunction {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(rename = "returnType")]
  pub return_type: String,
  #[serde(default)]
  pub params: Vec<CParam>,
}

/// A C function parameter.
#[derive(Debug, Deserialize)]
pub struct CParam {
  #[serde(rename = "type")]
  pub ty: String,
  #[serde(default)]
  pub name: String,
}

/// A C struct declaration.
#[derive(Debug, Deserialize)]
pub struct CStruct {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(default)]
  pub fields: Vec<CField>,
}

/// A C struct field.
#[derive(Debug, Deserialize)]
pub struct CField {
  #[serde(rename = "type")]
  pub ty: String,
  pub name: String,
  #[serde(default)]
  pub description: String,
}

/// A C enum declaration (only the name is needed today).
#[derive(Debug, Deserialize)]
pub struct CEnum {
  pub name: String,
  #[serde(default)]
  pub description: String,
}

/// A C type alias (`typedef Vector4 Quaternion;`).
#[derive(Debug, Deserialize)]
pub struct CAlias {
  pub name: String,
  #[serde(rename = "type")]
  pub ty: String,
}

/// Parse rlparser JSON into a [`CApi`].
pub fn parse_c_api(json: &str) -> Result<CApi, BindError> {
  serde_json::from_str(json).map_err(|err| BindError::Syntax(err.to_string()))
}
