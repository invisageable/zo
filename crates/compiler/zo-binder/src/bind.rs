//! Orchestrate parse → type-map → emit for one shim.

use crate::emit::Emitter;
use crate::model::{
  BindError, Bindings, FfiBinding, FfiItem, LinkSpec, ZoParam,
};
use crate::parse::parse_ffi_items;
use crate::tymap::map_ty;

/// Parse `src` and render the `.zo` binding file for `lib`.
pub fn bind(lib: &str, src: &str) -> Result<String, BindError> {
  let functions = parse_ffi_items(src)?
    .into_iter()
    .map(bind_item)
    .collect::<Result<Vec<_>, _>>()?;

  let bindings = Bindings {
    lib: lib.to_string(),
    link: LinkSpec::Provider,
    structs: vec![],
    functions,
  };

  Ok(Emitter::new().render(&bindings))
}

/// Type-map one parsed [`FfiItem`] into an [`FfiBinding`].
fn bind_item(item: FfiItem) -> Result<FfiBinding, BindError> {
  let FfiItem {
    name,
    params,
    ret,
    doc,
  } = item;

  let params = params
    .into_iter()
    .map(|param| {
      Ok(ZoParam {
        name: param.name,
        ty: map_ty(&param.ty, &name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  let ret = map_ty(&ret, &name)?;

  Ok(FfiBinding {
    name,
    link_name: None,
    params,
    ret,
    doc,
  })
}
