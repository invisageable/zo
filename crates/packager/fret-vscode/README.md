# Fret Configuration Syntax Highlighting

Syntax highlighting for `.oz` fret configuration files in Visual Studio Code.

## Features

- **Syntax Highlighting** for fret configuration files
- **Comment Support** with `--` line comments
- **Auto-closing** brackets and quotes
- **Code Folding** support
- **Namespace Support** for directives like `@pack:zo:release`

## Supported Syntax

```oz
-- This is a comment
@pack = (
  name: "my-project",
  version: "1.0.0",
  authors: ["invisageable"],
  license: "MIT",
)

-- Namespaced directives
@pack:zo:release = (
  name: "production-build",
  version: "2.0.0",
)
```

## Installation

### From Source

1. Navigate to the extension directory:
   ```bash
   cd crates/packager/fret-vscode
   ```

2. Copy the extension to your VS Code extensions folder:
   ```bash
   # macOS/Linux
   cp -r . ~/.vscode/extensions/fret-oz-syntax-0.1.0/

   # Windows
   xcopy /E /I . %USERPROFILE%\.vscode\extensions\fret-oz-syntax-0.1.0\
   ```

3. Reload VS Code or restart the editor

### Verify Installation

Open any `.oz` file and you should see syntax highlighting automatically applied.

## Language Features

### Syntax Elements

- **Directives**: `@pack` keyword with optional namespaces
- **Field Names**: Property names in the configuration
- **Strings**: Double-quoted strings with escape sequence support
- **Numbers**: Integer and decimal numbers
- **Booleans**: `true` and `false`
- **Arrays**: Square bracket notation `[...]`
- **Comments**: Line comments starting with `--`

### Auto-Completion

The extension provides:
- Auto-closing for `()`, `[]`, and `""`
- Bracket matching
- Comment toggling with `Cmd+/` (macOS) or `Ctrl+/` (Windows/Linux)

## Grammar Specification

Based on the fret configuration grammar:

```ebnf
directive = "@", "pack", { ":", identifier }, "=", value_tuple ;
value_tuple = "(", [ pair, { ",", pair } ], [ "," ], ")" ;
pair = identifier, ":", value ;
```

## Development

To modify the syntax highlighting:

1. Edit `syntaxes/fret-oz.tmLanguage.json` for grammar rules
2. Edit `language-configuration.json` for editor behavior
3. Reload VS Code to see changes

## License

Part of the zo compiler ecosystem.
