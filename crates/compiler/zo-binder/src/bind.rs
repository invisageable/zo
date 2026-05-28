//! Orchestrate parse → type-map → emit for one shim.

use crate::emit::Emitter;
use crate::model::{BindError, FfiBinding, FfiItem, ZoParam};
use crate::parse::parse_ffi_items;
use crate::tymap::map_ty;

/// Parse `src` and render the `.zo` binding file for `lib`.
pub fn bind(lib: &str, src: &str) -> Result<String, BindError> {
  let items = parse_ffi_items(src)?;
  let bindings = items.iter().map(bind_item).collect::<Result<Vec<_>, _>>()?;

  Ok(Emitter::new().render(lib, &bindings))
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
    params,
    ret: map_ty(&item.ret, &item.name)?,
    doc: item.doc.clone(),
  })
}
