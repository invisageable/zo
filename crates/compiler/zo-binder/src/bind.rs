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
    .iter()
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
fn bind_item(item: &FfiItem) -> Result<FfiBinding, BindError> {
  let params = item
    .params
    .iter()
    .map(|param| {
      Ok(ZoParam {
        name: param.name.clone(),
        ty: map_ty(&param.ty, &item.name)?,
      })
    })
    .collect::<Result<Vec<_>, BindError>>()?;

  Ok(FfiBinding {
    name: item.name.clone(),
    link_name: None,
    params,
    ret: map_ty(&item.ret, &item.name)?,
    doc: item.doc.clone(),
  })
}
