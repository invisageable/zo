// A suffixes dictionnary.
pub fn suffixes() -> std::collections::HashSet<&'static str> {
  std::collections::HashSet::from([
    "int", "s8", "s16", "s32", "s64", "s128", "u8", "u16", "u32", "u64",
    "u128", "float", "f32", "f64", "char", "str",
  ])
}
