use crate::cheader::parse_c_api;

/// Parse a sample in rlparser's `raylib_api.json` shape and
/// confirm the model captures functions, structs, enums, and
/// aliases with the right field names.
#[test]
fn parses_rlparser_json_shape() {
  let json = r#"{
    "functions": [
      {
        "name": "InitWindow",
        "description": "Initialize window and OpenGL context",
        "returnType": "void",
        "params": [
          { "type": "int", "name": "width" },
          { "type": "const char *", "name": "title" }
        ]
      }
    ],
    "structs": [
      {
        "name": "Vector2",
        "description": "Vector2, 2 components",
        "fields": [
          { "type": "float", "name": "x", "description": "x" },
          { "type": "float", "name": "y", "description": "y" }
        ]
      }
    ],
    "enums": [
      { "name": "ConfigFlags", "description": "System/Window flags" }
    ],
    "aliases": [
      { "type": "Vector4", "name": "Quaternion", "description": "alias" }
    ]
  }"#;

  let api = parse_c_api(json).unwrap();

  assert_eq!(api.functions.len(), 1);
  assert_eq!(api.functions[0].name, "InitWindow");
  assert_eq!(api.functions[0].return_type, "void");
  assert_eq!(api.functions[0].params[1].ty, "const char *");
  assert_eq!(api.functions[0].params[1].name, "title");

  assert_eq!(api.structs[0].name, "Vector2");
  assert_eq!(api.structs[0].fields.len(), 2);

  assert_eq!(api.enums[0].name, "ConfigFlags");
  assert_eq!(api.aliases[0].name, "Quaternion");
}
