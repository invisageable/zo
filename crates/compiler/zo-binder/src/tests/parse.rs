use crate::model::{Param, RustTy};
use crate::parse::parse_ffi_items;

/// `extern "C"` + `#[no_mangle]` functions are kept; plain
/// and private functions are skipped.
#[test]
fn keeps_only_ffi_functions() {
  let src = r#"
    #[no_mangle]
    pub extern "C" fn demo_new() -> i64 { 0 }

    #[no_mangle]
    pub extern "C" fn demo_push(stack: i64, value: i64) {}

    pub fn helper() -> i64 { 0 }

    fn private() {}
  "#;

  let items = parse_ffi_items(src).unwrap();
  let names: Vec<_> = items.iter().map(|item| item.name.as_str()).collect();

  assert_eq!(names, vec!["demo_new", "demo_push"]);
}

/// The 2024 `#[unsafe(no_mangle)]` form is recognized.
#[test]
fn accepts_unsafe_no_mangle() {
  let src = r#"
    #[unsafe(no_mangle)]
    pub extern "C" fn f(x: i64) -> i64 { x }
  "#;

  let items = parse_ffi_items(src).unwrap();

  assert_eq!(items.len(), 1);
  assert_eq!(items[0].name, "f");
}

/// `#[no_mangle]` without `extern "C"` is not an FFI export.
#[test]
fn skips_no_mangle_without_extern_c() {
  let src = r#"
    #[no_mangle]
    pub fn f() {}
  "#;

  assert!(parse_ffi_items(src).unwrap().is_empty());
}

/// Pointers normalize to `Ptr`, an absent return to `Unit`.
#[test]
fn normalizes_pointers_and_unit_return() {
  let src = r#"
    #[no_mangle]
    pub extern "C" fn f(p: *const c_char, q: *mut u8) {}
  "#;

  let items = parse_ffi_items(src).unwrap();
  let fun = &items[0];

  assert_eq!(
    fun.params,
    vec![
      Param {
        name: "p".to_string(),
        ty: RustTy::Ptr {
          mutable: false,
          inner: Box::new(RustTy::Path("c_char".to_string())),
        },
      },
      Param {
        name: "q".to_string(),
        ty: RustTy::Ptr {
          mutable: true,
          inner: Box::new(RustTy::Path("u8".to_string())),
        },
      },
    ]
  );
  assert_eq!(fun.ret, RustTy::Unit);
}

/// A named return type keeps its last path segment.
#[test]
fn captures_named_return_type() {
  let src = r#"
    #[no_mangle]
    pub extern "C" fn f() -> std::os::raw::c_int { 0 }
  "#;

  let items = parse_ffi_items(src).unwrap();

  assert_eq!(items[0].ret, RustTy::Path("c_int".to_string()));
}

/// `///` doc lines are captured, trimmed, in order.
#[test]
fn captures_doc_lines() {
  let src = r#"
    /// First line.
    /// Second line.
    #[no_mangle]
    pub extern "C" fn f() {}
  "#;

  let items = parse_ffi_items(src).unwrap();

  assert_eq!(items[0].doc, vec!["First line.", "Second line."]);
}

/// `type X = Y;` aliases resolve to their concrete type.
#[test]
fn resolves_type_aliases() {
  let src = r#"
    type ZoHandle = i64;

    #[no_mangle]
    pub extern "C" fn f(h: ZoHandle) -> ZoHandle { h }
  "#;

  let items = parse_ffi_items(src).unwrap();

  assert_eq!(items[0].params[0].ty, RustTy::Path("i64".to_string()));
  assert_eq!(items[0].ret, RustTy::Path("i64".to_string()));
}
