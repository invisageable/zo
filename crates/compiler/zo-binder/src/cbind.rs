//! Build bindings from a parsed C API (rlparser JSON).

use crate::cheader::{CApi, CFunction, CStruct};
use crate::ctymap::map_c_ty;
use crate::emit::Emitter;
use crate::model::{
  BindError, Bindings, FfiBinding, LinkSpec, ZoField, ZoParam, ZoStruct,
};

use swisskit_core::to;

use std::collections::HashSet;

/// Render zo bindings for `lib` from a parsed C API.
pub fn bind_c_api(
  lib: &str,
  link: LinkSpec,
  api: &CApi,
) -> Result<String, BindError> {
  let known = known_types(api);

  let structs = api
    .structs
    .iter()
    .map(|item| bind_struct(item, &known))
    .collect::<Result<Vec<_>, _>>()?;

  let functions = api
    .functions
    .iter()
    .map(|function| bind_function(function, &known))
    .collect::<Result<Vec<_>, _>>()?;

  let bindings = Bindings {
    lib: lib.to_string(),
    link,
    structs,
    functions,
  };

  Ok(Emitter::new().render(&bindings))
}

/// Names the type mapper may resolve to a zo struct.
fn known_types(api: &CApi) -> HashSet<String> {
  api
    .structs
    .iter()
    .map(|item| item.name.clone())
    .chain(api.enums.iter().map(|item| item.name.clone()))
    .chain(api.aliases.iter().map(|item| item.name.clone()))
    .collect()
}

/// Type-map one C function into an [`FfiBinding`].
fn bind_function(
  function: &CFunction,
  known: &HashSet<String>,
) -> Result<FfiBinding, BindError> {
  let name = to!(snake &function.name);
  let link_name = (name != function.name).then(|| function.name.clone());

  let params = function
    .params
    .iter()
    .filter(|param| param.ty != "void")
    .map(|param| {
      Ok(ZoParam {
        name: to!(snake &param.name),
        ty: map_c_ty(&param.ty, known, &function.name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  Ok(FfiBinding {
    name,
    link_name,
    params,
    ret: map_c_ty(&function.return_type, known, &function.name)?,
    doc: doc(&function.description),
  })
}

/// Map one C struct into a [`ZoStruct`].
fn bind_struct(
  item: &CStruct,
  known: &HashSet<String>,
) -> Result<ZoStruct, BindError> {
  let fields = item
    .fields
    .iter()
    .map(|field| {
      Ok(ZoField {
        name: to!(snake &field.name),
        ty: map_c_ty(&field.ty, known, &item.name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  Ok(ZoStruct {
    name: item.name.clone(),
    fields,
    doc: doc(&item.description),
  })
}

/// One doc line from a description, or none when empty.
fn doc(description: &str) -> Vec<String> {
  if description.is_empty() {
    vec![]
  } else {
    vec![description.to_string()]
  }
}
