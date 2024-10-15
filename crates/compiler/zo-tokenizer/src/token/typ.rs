use hashbrown::HashSet;
use smol_str::SmolStr;

/// The type dictionnary.
type Types = HashSet<SmolStr>;

lazy_static::lazy_static! {
    // reserved words for types.
    pub static ref TYPES: Types = HashSet::from([
      (SmolStr::new_inline("int")),
      (SmolStr::new_inline("s8")),
      (SmolStr::new_inline("s16")),
      (SmolStr::new_inline("s32")),
      (SmolStr::new_inline("s64")),
      (SmolStr::new_inline("s128")),
      (SmolStr::new_inline("u8")),
      (SmolStr::new_inline("u16")),
      (SmolStr::new_inline("u32")),
      (SmolStr::new_inline("u64")),
      (SmolStr::new_inline("u128")),
      (SmolStr::new_inline("float")),
      (SmolStr::new_inline("f32")),
      (SmolStr::new_inline("f64")),
      (SmolStr::new_inline("char")),
      (SmolStr::new_inline("str")),
    ]);
}
