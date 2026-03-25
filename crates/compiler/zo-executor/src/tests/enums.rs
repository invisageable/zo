use crate::tests::common::assert_sir_structure;

use zo_sir::Insn;

#[test]
fn test_enum_basic() {
  assert_sir_structure(
    r#"enum Foo {
  Ok,
  Err,
}"#,
    |sir| {
      let enum_def = sir.iter().find(|i| matches!(i, Insn::EnumDef { .. }));

      assert!(enum_def.is_some(), "expected EnumDef instruction");

      if let Some(Insn::EnumDef { variants, .. }) = enum_def {
        assert_eq!(
          variants.len(),
          2,
          "expected 2 variants, got {}",
          variants.len()
        );
      }
    },
  );
}

#[test]
fn test_enum_tuple_variant() {
  assert_sir_structure(
    r#"enum Foo {
  Ok(int),
  Err(int),
}"#,
    |sir| {
      let enum_def = sir.iter().find(|i| matches!(i, Insn::EnumDef { .. }));

      if let Some(Insn::EnumDef { variants, .. }) = enum_def {
        assert_eq!(variants.len(), 2);

        // Both variants have 1 field.
        assert_eq!(variants[0].2.len(), 1, "Ok should have 1 field");
        assert_eq!(variants[1].2.len(), 1, "Err should have 1 field");
      } else {
        panic!("expected EnumDef");
      }
    },
  );
}

#[test]
fn test_enum_discriminant() {
  assert_sir_structure(
    r#"enum Bar {
  FooBar,
  RabOof = 0,
}"#,
    |sir| {
      let enum_def = sir.iter().find(|i| matches!(i, Insn::EnumDef { .. }));

      if let Some(Insn::EnumDef { variants, .. }) = enum_def {
        assert_eq!(variants.len(), 2);

        // FooBar: auto discriminant = 0.
        assert_eq!(variants[0].1, 0);
        // RabOof: explicit discriminant = 0.
        assert_eq!(variants[1].1, 0);
      } else {
        panic!("expected EnumDef");
      }
    },
  );
}

#[test]
fn test_enum_unit_variants_auto_discriminant() {
  assert_sir_structure(
    r#"enum Color {
  Red,
  Green,
  Blue,
}"#,
    |sir| {
      let enum_def = sir.iter().find(|i| matches!(i, Insn::EnumDef { .. }));

      if let Some(Insn::EnumDef { variants, .. }) = enum_def {
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].1, 0); // Red
        assert_eq!(variants[1].1, 1); // Green
        assert_eq!(variants[2].1, 2); // Blue
      } else {
        panic!("expected EnumDef");
      }
    },
  );
}

#[test]
fn test_enum_mixed_variants() {
  assert_sir_structure(
    r#"enum Result {
  Ok,
  Err(int),
}"#,
    |sir| {
      let enum_def = sir.iter().find(|i| matches!(i, Insn::EnumDef { .. }));

      if let Some(Insn::EnumDef { variants, .. }) = enum_def {
        assert_eq!(variants.len(), 2);
        // Ok: unit variant, 0 fields.
        assert_eq!(variants[0].2.len(), 0);
        // Err: tuple variant, 1 field.
        assert_eq!(variants[1].2.len(), 1);
      } else {
        panic!("expected EnumDef");
      }
    },
  );
}

// === ENUM CONSTRUCTION ===

#[test]
fn test_enum_construct_unit() {
  assert_sir_structure(
    r#"enum Color { Red, Green, Blue }
fun main() {
  imu c = Color::Red;
}"#,
    |sir| {
      let construct =
        sir.iter().find(|i| matches!(i, Insn::EnumConstruct { .. }));

      assert!(construct.is_some(), "expected EnumConstruct for Color::Red");

      if let Some(Insn::EnumConstruct {
        variant, fields, ..
      }) = construct
      {
        assert_eq!(*variant, 0, "Red is discriminant 0");
        assert!(fields.is_empty(), "unit has no fields");
      }
    },
  );
}

#[test]
fn test_enum_construct_tuple() {
  assert_sir_structure(
    r#"enum Foo { Ok(int), Err(int) }
fun main() {
  imu x = Foo::Ok(42);
}"#,
    |sir| {
      let construct =
        sir.iter().find(|i| matches!(i, Insn::EnumConstruct { .. }));

      assert!(
        construct.is_some(),
        "expected EnumConstruct for Foo::Ok(42)"
      );

      if let Some(Insn::EnumConstruct {
        variant, fields, ..
      }) = construct
      {
        assert_eq!(*variant, 0, "Ok is discriminant 0");
        assert_eq!(fields.len(), 1, "Ok has 1 field");
      }
    },
  );
}

// === APPLY ON ENUMS ===

#[test]
fn test_apply_enum_static() {
  assert_sir_structure(
    r#"enum Color { Red, Green, Blue }
apply Color {
  fun red() -> Self {
    Self::Red
  }
}
fun main() {
  imu c := Color::red();
}"#,
    |sir| {
      // Should have a FunDef with mangled name.
      let method = sir.iter().find(|i| matches!(i, Insn::Call { .. }));

      assert!(method.is_some(), "expected Call for Color::red()");
    },
  );
}
