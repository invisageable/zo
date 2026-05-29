use crate::ctymap::map_c_ty;
use crate::model::{BindError, ZoTy};

use std::collections::HashSet;

/// A set of known struct/enum/alias names.
fn known(names: &[&str]) -> HashSet<String> {
  names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn maps_c_primitives() {
  let types = known(&[]);

  assert_eq!(map_c_ty("void", &types, "f").unwrap(), ZoTy::Unit);
  assert_eq!(map_c_ty("int", &types, "f").unwrap(), ZoTy::Named("int"));
  assert_eq!(
    map_c_ty("unsigned int", &types, "f").unwrap(),
    ZoTy::Named("uint")
  );
  assert_eq!(
    map_c_ty("float", &types, "f").unwrap(),
    ZoTy::Named("float")
  );
  assert_eq!(map_c_ty("double", &types, "f").unwrap(), ZoTy::Named("f64"));
  assert_eq!(map_c_ty("bool", &types, "f").unwrap(), ZoTy::Named("bool"));
}

#[test]
fn maps_c_strings_and_pointers() {
  let types = known(&[]);

  assert_eq!(
    map_c_ty("const char *", &types, "f").unwrap(),
    ZoTy::Named("CStr")
  );
  assert_eq!(
    map_c_ty("char *", &types, "f").unwrap(),
    ZoTy::Named("CStr")
  );
  assert_eq!(map_c_ty("void *", &types, "f").unwrap(), ZoTy::Named("s64"));
  assert_eq!(
    map_c_ty("Camera3D *", &types, "f").unwrap(),
    ZoTy::Named("s64")
  );
}

#[test]
fn maps_known_named_types_to_structs() {
  let types = known(&["Vector2", "Color"]);

  assert_eq!(
    map_c_ty("Vector2", &types, "f").unwrap(),
    ZoTy::Struct("Vector2".to_string())
  );
  assert_eq!(
    map_c_ty("Color", &types, "f").unwrap(),
    ZoTy::Struct("Color".to_string())
  );
}

#[test]
fn rejects_unknown_named_type() {
  let types = known(&[]);
  let error = map_c_ty("Wibble", &types, "draw").unwrap_err();

  assert_eq!(
    error,
    BindError::UnsupportedType {
      func: "draw".to_string(),
      rust_ty: "Wibble".to_string(),
    }
  );
}
