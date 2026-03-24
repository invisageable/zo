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
