use crate::model::{BindError, RustTy, ZoTy};
use crate::tymap::map_ty;

/// A named-path Rust type.
fn path(name: &str) -> RustTy {
  RustTy::Path(name.to_string())
}

/// A pointer Rust type.
fn ptr(mutable: bool, inner: RustTy) -> RustTy {
  RustTy::Ptr {
    mutable,
    inner: Box::new(inner),
  }
}

#[test]
fn maps_signed_integers() {
  assert_eq!(map_ty(&path("i8"), "f").unwrap(), ZoTy::Named("s8"));
  assert_eq!(map_ty(&path("i16"), "f").unwrap(), ZoTy::Named("s16"));
  assert_eq!(map_ty(&path("i32"), "f").unwrap(), ZoTy::Named("int"));
  assert_eq!(map_ty(&path("i64"), "f").unwrap(), ZoTy::Named("s64"));
  assert_eq!(map_ty(&path("isize"), "f").unwrap(), ZoTy::Named("s64"));
}

#[test]
fn maps_unsigned_integers() {
  assert_eq!(map_ty(&path("u8"), "f").unwrap(), ZoTy::Named("u8"));
  assert_eq!(map_ty(&path("u16"), "f").unwrap(), ZoTy::Named("u16"));
  assert_eq!(map_ty(&path("u32"), "f").unwrap(), ZoTy::Named("uint"));
  assert_eq!(map_ty(&path("u64"), "f").unwrap(), ZoTy::Named("u64"));
  assert_eq!(map_ty(&path("usize"), "f").unwrap(), ZoTy::Named("u64"));
}

#[test]
fn maps_floats_and_bool() {
  assert_eq!(map_ty(&path("f32"), "f").unwrap(), ZoTy::Named("float"));
  assert_eq!(map_ty(&path("f64"), "f").unwrap(), ZoTy::Named("f64"));
  assert_eq!(map_ty(&path("bool"), "f").unwrap(), ZoTy::Named("bool"));
}

#[test]
fn maps_unit() {
  assert_eq!(map_ty(&RustTy::Unit, "f").unwrap(), ZoTy::Unit);
}

#[test]
fn maps_c_char_pointer_to_cstr() {
  let ty = ptr(false, path("c_char"));

  assert_eq!(map_ty(&ty, "f").unwrap(), ZoTy::Named("CStr"));
}

#[test]
fn maps_other_pointers_to_s64() {
  let raw = ptr(true, path("u8"));
  let opaque = ptr(false, path("MyStruct"));

  assert_eq!(map_ty(&raw, "f").unwrap(), ZoTy::Named("s64"));
  assert_eq!(map_ty(&opaque, "f").unwrap(), ZoTy::Named("s64"));
}

#[test]
fn rejects_unknown_path_with_spelling() {
  let error = map_ty(&path("String"), "do_thing").unwrap_err();

  assert_eq!(
    error,
    BindError::UnsupportedType {
      func: "do_thing".to_string(),
      rust_ty: "String".to_string(),
    }
  );
}

#[test]
fn rejects_other_type_with_spelling() {
  let ty = RustTy::Other("&str".to_string());
  let error = map_ty(&ty, "do_thing").unwrap_err();

  assert_eq!(
    error,
    BindError::UnsupportedType {
      func: "do_thing".to_string(),
      rust_ty: "&str".to_string(),
    }
  );
}
