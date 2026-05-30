//! Build bindings from a parsed C API (rlparser JSON).

use crate::cheader::{CApi, CFunction, CStruct};
use crate::ctymap::map_c_ty;
use crate::emit::Emitter;
use crate::model::{
  BindError, Bindings, FfiBinding, LinkSpec, ZoField, ZoParam, ZoStruct, ZoTy,
};

use swisskit_core::to;

use std::collections::{HashMap, HashSet};

/// The result of binding a C API: the rendered file plus the
/// names skipped because a type did not map (callbacks, etc.).
#[derive(Debug)]
pub struct CBindResult {
  /// The rendered `.zo` source.
  pub output: String,
  /// Names skipped because a type did not map.
  pub skipped: Vec<String>,
}

/// Render zo bindings for `lib` from a parsed C API.
///
/// @note — a function or struct whose types do not map is
/// skipped (not an error), keeping the generated surface
/// self-consistent: both structs and functions bind only
/// against structs that themselves generated.
pub fn bind_c_api(lib: &str, link: LinkSpec, api: &CApi) -> CBindResult {
  let aliases = alias_map(api);

  let mut structs = Vec::new();
  let mut generated = HashSet::new();
  let mut skipped = Vec::new();

  for item in &api.structs {
    match bind_struct(item, &generated, &aliases) {
      Ok(zo_struct) => {
        generated.insert(item.name.clone());
        structs.push(zo_struct);
      }
      Err(_) => skipped.push(format!("struct {}", item.name)),
    }
  }

  let mut functions = Vec::new();

  for function in &api.functions {
    match bind_function(function, &generated, &aliases) {
      Ok(binding) => functions.push(binding),
      Err(_) => skipped.push(function.name.clone()),
    }
  }

  let bindings = Bindings {
    lib: lib.to_string(),
    link,
    structs,
    functions,
  };

  CBindResult {
    output: Emitter::new().render(&bindings),
    skipped,
  }
}

/// Alias name → target type (`Texture2D` → `Texture`).
fn alias_map(api: &CApi) -> HashMap<String, String> {
  api
    .aliases
    .iter()
    .map(|alias| (alias.name.clone(), alias.ty.clone()))
    .collect()
}

/// Type-map one C function into an [`FfiBinding`].
fn bind_function(
  function: &CFunction,
  structs: &HashSet<String>,
  aliases: &HashMap<String, String>,
) -> Result<FfiBinding, BindError> {
  let name = to!(snake & function.name);
  let link_name = (name != function.name).then(|| function.name.clone());

  let params = function
    .params
    .iter()
    .filter(|param| param.ty != "void")
    .map(|param| {
      Ok(ZoParam {
        name: to!(snake & param.name),
        ty: map_type(&param.ty, structs, aliases, &function.name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  Ok(FfiBinding {
    name,
    link_name,
    params,
    ret: map_type(&function.return_type, structs, aliases, &function.name)?,
    doc: doc(&function.description),
  })
}

/// Map one C struct into a [`ZoStruct`].
fn bind_struct(
  item: &CStruct,
  structs: &HashSet<String>,
  aliases: &HashMap<String, String>,
) -> Result<ZoStruct, BindError> {
  let fields = item
    .fields
    .iter()
    .map(|field| {
      Ok(ZoField {
        name: to!(snake & field.name),
        ty: map_type(&field.ty, structs, aliases, &item.name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  Ok(ZoStruct {
    name: item.name.clone(),
    fields,
    doc: doc(&item.description),
  })
}

/// Resolve a one-level alias, then map the C type to a zo type.
fn map_type(
  c: &str,
  structs: &HashSet<String>,
  aliases: &HashMap<String, String>,
  func: &str,
) -> Result<ZoTy, BindError> {
  let resolved = aliases.get(c.trim()).map(String::as_str).unwrap_or(c);

  map_c_ty(resolved, structs, func)
}

/// One doc line from a description, or none when empty.
fn doc(description: &str) -> Vec<String> {
  if description.is_empty() {
    vec![]
  } else {
    vec![description.to_string()]
  }
}
