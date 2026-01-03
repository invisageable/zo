{
  "targets": [
    {
      "target_name": "tree_sitter_zo_binding",
      "dependencies": [
        "<!(node -p \"require('node-addon-api').targets\"):node_addon_api_except",
      ],
      "include_dirs": [
        "src",
      ],
      "sources": [
        "bindings/node/binding.cc",
        "src/parser.c",
        "src/scanner.c",
      ],
      "cflags_c": [
        "-std=c11",
        "-fvisibility=hidden",
      ],
      "cflags_cc": [
        "-fvisibility=hidden",
      ],
      "xcode_settings": {
        "CLANG_CXX_LANGUAGE_STANDARD": "c++17",
        "MACOSX_DEPLOYMENT_TARGET": "10.15",
        "GCC_SYMBOLS_PRIVATE_EXTERN": "YES",
      },
    }
  ]
}
