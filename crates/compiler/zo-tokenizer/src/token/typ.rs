use compact_str::CompactString;
use hashbrown::HashSet;

/// The type dictionnary.
type Types = HashSet<CompactString>;

lazy_static::lazy_static! {
    // reserved words for types.
    pub static ref TYPES: Types = HashSet::from([
      (CompactString::const_new("int")),
      (CompactString::const_new("s8")),
      (CompactString::const_new("s16")),
      (CompactString::const_new("s32")),
      (CompactString::const_new("s64")),
      (CompactString::const_new("s128")),
      (CompactString::const_new("u8")),
      (CompactString::const_new("u16")),
      (CompactString::const_new("u32")),
      (CompactString::const_new("u64")),
      (CompactString::const_new("u128")),
      (CompactString::const_new("float")),
      (CompactString::const_new("f32")),
      (CompactString::const_new("f64")),
      (CompactString::const_new("char")),
      (CompactString::const_new("str")),
    ]);
}
