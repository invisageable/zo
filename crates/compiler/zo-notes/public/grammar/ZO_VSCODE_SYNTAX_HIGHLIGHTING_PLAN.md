# VS Code Syntax Highlighting Implementation Plan for Zo Language

## Executive Summary

This document outlines a precise, production-ready plan to implement syntax highlighting for the Zo programming language in VS Code using TextMate grammars. The implementation will support all Zo language features including templates (ZSX), directives, typestate, and structured concurrency.

## 1. Project Structure

```
zo-vscode/
├── package.json                    # Extension manifest
├── README.md                        # Extension documentation
├── CHANGELOG.md                     # Version history
├── LICENSE                          # MIT License
├── .vscodeignore                    # Files to exclude from package
├── .gitignore                       # Git ignore patterns
├── syntaxes/
│   ├── zo.tmLanguage.json         # Main Zo grammar
│   └── zo.injection.json          # Template/ZSX injection grammar
├── language-configuration.json     # Language configuration
├── snippets/
│   └── zo.snippets.json           # Code snippets
├── themes/                         # Optional custom themes
│   └── zo-dark.json                # Zo-optimized dark theme
└── test/
    └── samples/
        ├── basic.zo                # Test file for basic syntax
        ├── templates.zo            # Test file for ZSX templates
        └── advanced.zo             # Test file for advanced features
```

## 2. Language Configuration

### 2.1 package.json Configuration

```json
{
  "name": "zo-lang",
  "displayName": "Zo Language Support",
  "description": "Syntax highlighting and language support for Zo programming language",
  "version": "0.1.0",
  "publisher": "zo-team",
  "engines": {
    "vscode": "^1.74.0"
  },
  "categories": ["Programming Languages"],
  "contributes": {
    "languages": [{
      "id": "zo",
      "aliases": ["Zo", "zo"],
      "extensions": [".zo"],
      "configuration": "./language-configuration.json"
    }],
    "grammars": [{
      "language": "zo",
      "scopeName": "source.zo",
      "path": "./syntaxes/zo.tmLanguage.json"
    }, {
      "scopeName": "text.html.zo",
      "path": "./syntaxes/zo.injection.json",
      "injectTo": ["source.zo"],
      "embeddedLanguages": {
        "meta.embedded.block.html": "html"
      }
    }]
  }
}
```

### 2.2 language-configuration.json

```json
{
  "comments": {
    "lineComment": "--",
    "blockComment": ["-*", "*-"]
  },
  "brackets": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
    ["<", ">"]
  ],
  "autoClosingPairs": [
    { "open": "{", "close": "}" },
    { "open": "[", "close": "]" },
    { "open": "(", "close": ")" },
    { "open": "\"", "close": "\"", "notIn": ["string"] },
    { "open": "'", "close": "'", "notIn": ["string", "comment"] },
    { "open": "<", "close": ">", "notIn": ["string", "comment"] },
    { "open": "$\"", "close": "\"", "notIn": ["string"] }
  ],
  "surroundingPairs": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
    ["\"", "\""],
    ["'", "'"],
    ["<", ">"]
  ],
  "folding": {
    "markers": {
      "start": "^\\s*-- region\\b",
      "end": "^\\s*-- endregion\\b"
    }
  },
  "wordPattern": "(-?\\d*\\.\\d\\w*)|([^\\`\\~\\!\\@\\#\\%\\^\\&\\*\\(\\)\\-\\=\\+\\[\\{\\]\\}\\\\\\|\\;\\:\\'\\\"\\,\\.\\<\\>\\?\\/\\s]+)",
  "indentationRules": {
    "increaseIndentPattern": "^\\s*(fun|struct|enum|if|else|while|for|loop|match|nursery|abstract|apply|pack|group).*\\{\\s*$",
    "decreaseIndentPattern": "^\\s*\\}.*$"
  }
}
```

## 3. TextMate Grammar Structure

### 3.1 Core Grammar Components

The main `zo.tmLanguage.json` will have these top-level sections:

```json
{
  "scopeName": "source.zo",
  "name": "Zo",
  "fileTypes": ["zo"],
  "patterns": [
    { "include": "#comments" },
    { "include": "#directives" },
    { "include": "#attributes" },
    { "include": "#declarations" },
    { "include": "#statements" },
    { "include": "#expressions" }
  ],
  "repository": {
    // All pattern definitions
  }
}
```

### 3.2 Token Categories and Scopes

#### Comments
- `comment.line.double-dash.zo` - Line comments (`--`)
- `comment.line.documentation.zo` - Doc comments (`-!`)
- `comment.block.zo` - Block comments (`-*` ... `*-`)

#### Keywords
- `keyword.control.zo` - Control flow (`if`, `else`, `while`, `for`, `loop`, `match`, `when`)
- `keyword.control.flow.zo` - Flow control (`return`, `break`, `continue`, `await`)
- `keyword.declaration.zo` - Declarations (`fun`, `struct`, `enum`, `type`, `alias`, `pack`, `load`)
- `keyword.modifier.zo` - Modifiers (`pub`, `raw`, `wasm`, `imu`, `mut`, `val`, `ext`)
- `keyword.operator.zo` - Operator keywords (`is`, `as`, `and`, `or`)
- `keyword.other.zo` - Other keywords (`abstract`, `apply`, `group`, `nursery`, `spawn`, `await`, `shift`, `state`, `self`, `Self`, `for`, `type@state`)

#### Types
- `storage.type.primitive.zo` - Primitive types (`int`, `float`, `bool`, `char`, `str`, `void`)
- `storage.type.numeric.zo` - Numeric types (`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `f64`)
- `entity.name.type.zo` - User-defined types
- `storage.type.template.zo` - Template type (`</>`)
- `storage.type.generic.zo` - Generic types (`$T`, `$U`)

#### Literals
- `constant.numeric.integer.decimal.zo` - Decimal integer literals
- `constant.numeric.integer.binary.zo` - Binary literals (`0b`)
- `constant.numeric.integer.octal.zo` - Octal literals (`0o`)
- `constant.numeric.integer.hex.zo` - Hex literals (`0x`)
- `constant.numeric.integer.base.zo` - Base literals (`b#`, `o#`, `x#`)
- `constant.numeric.float.zo` - Float literals with optional exponent
- `constant.language.boolean.zo` - Boolean literals (`true`, `false`)
- `string.quoted.single.zo` - Character literals
- `string.quoted.double.zo` - String literals
- `string.quoted.raw.zo` - Raw string literals (`$"..."`)
- `constant.other.bytes.zo` - Bytes literals (`` ` ``)

#### Operators
- `keyword.operator.arithmetic.zo` - Arithmetic operators (`+`, `-`, `*`, `/`, `%`)
- `keyword.operator.comparison.zo` - Comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`)
- `keyword.operator.logical.zo` - Logical operators (`&&`, `||`, `!`)
- `keyword.operator.bitwise.zo` - Bitwise operators (`&`, `|`, `^`, `<<`, `>>`)
- `keyword.operator.assignment.zo` - Assignment operators (`=`, `+=`, `-=`, etc.)
- `keyword.operator.range.zo` - Range operators (`..`, `..=`)
- `keyword.operator.arrow.zo` - Arrow operators (`->`, `->>`, `=>`,)
  - `->` for return types and closure expressions
  - `=>` for match arms and single-line statements
  - `|>` for typestate transitions
- `keyword.operator.type.zo` - Type operators (`:`, `::=`, `:=`)

#### Functions and Identifiers
- `entity.name.function.zo` - Function names
- `variable.parameter.zo` - Function parameters
- `variable.other.zo` - Variable names
- `entity.name.namespace.zo` - Package/module names

#### Special Constructs
- `meta.preprocessor.zo` - Directives (`#run`, `#dom`, `#if`)
- `meta.attribute.zo` - Attributes (`%%`)
- `meta.embedded.block.zsx` - ZSX/template blocks
- `meta.typestate.zo` - Typestate declarations

### 3.3 Complex Pattern Examples

#### Comment Patterns
```json
{
  "comments": {
    "patterns": [
      {
        "name": "comment.line.documentation.zo",
        "match": "(-!).*$",
        "captures": {
          "1": { "name": "punctuation.definition.comment.zo" }
        }
      },
      {
        "name": "comment.line.double-dash.zo",
        "match": "(--).*$",
        "captures": {
          "1": { "name": "punctuation.definition.comment.zo" }
        }
      },
      {
        "name": "comment.block.zo",
        "begin": "-\\*",
        "end": "\\*-",
        "beginCaptures": {
          "0": { "name": "punctuation.definition.comment.begin.zo" }
        },
        "endCaptures": {
          "0": { "name": "punctuation.definition.comment.end.zo" }
        }
      }
    ]
  }
}

#### Function Declaration Pattern
```json
{
  "name": "meta.function.zo",
  "begin": "\\b(pub\\s+)?(raw\\s+)?(wasm\\s+)?(fun)\\s+([a-zA-Z_][a-zA-Z0-9_]*)",
  "beginCaptures": {
    "1": { "name": "keyword.modifier.visibility.zo" },
    "2": { "name": "keyword.modifier.raw.zo" },
    "3": { "name": "keyword.modifier.wasm.zo" },
    "4": { "name": "keyword.declaration.function.zo" },
    "5": { "name": "entity.name.function.zo" }
  },
  "end": "(?<=\\})|;",
  "patterns": [
    { "include": "#generic-parameters" },
    { "include": "#function-parameters" },
    { "include": "#return-type" },
    { "include": "#function-body" }
  ]
}
```

#### Closure Pattern
```json
{
  "name": "meta.function.closure.zo",
  "begin": "\\b(fn)\\s*\\(",
  "beginCaptures": {
    "1": { "name": "keyword.declaration.function.zo" }
  },
  "end": "(?<=\\})|(?<=;)|(?<=[^=]>)",
  "patterns": [
    { "include": "#function-parameters" },
    {
      "match": "(:)\\s*([^=]+?)\\s*(=>)",
      "captures": {
        "1": { "name": "punctuation.separator.type.zo" },
        "2": { "include": "#types" },
        "3": { "name": "keyword.operator.arrow.zo" }
      }
    },
    { "include": "#function-body" }
  ]
}
```

#### Template/ZSX Pattern
```json
{
  "name": "meta.embedded.block.zsx",
  "begin": "(<)(/?)([a-zA-Z][a-zA-Z0-9_-]*)|(<>)",
  "beginCaptures": {
    "1": { "name": "punctuation.definition.tag.begin.zo" },
    "2": { "name": "punctuation.definition.tag.close.zo" },
    "3": { "name": "entity.name.tag.zo" },
    "4": { "name": "punctuation.definition.fragment.begin.zo" }
  },
  "end": "(/?)>|(</>)",
  "endCaptures": {
    "1": { "name": "punctuation.definition.tag.self-close.zo" },
    "2": { "name": "punctuation.definition.fragment.end.zo" }
  },
  "patterns": [
    { "include": "#zsx-attributes" },
    { "include": "#zsx-interpolation" }
  ]
}
```

#### Literal Patterns
```json
{
  "literals": {
    "patterns": [
      {
        "name": "constant.numeric.integer.binary.zo",
        "match": "\\b0b[01][01_]*\\b"
      },
      {
        "name": "constant.numeric.integer.octal.zo",
        "match": "\\b0o[0-7][0-7_]*\\b"
      },
      {
        "name": "constant.numeric.integer.hex.zo",
        "match": "\\b0x[0-9a-fA-F][0-9a-fA-F_]*\\b"
      },
      {
        "name": "constant.numeric.integer.base.binary.zo",
        "match": "\\bb#[0-9]+\\b"
      },
      {
        "name": "constant.numeric.integer.base.octal.zo",
        "match": "\\bo#[0-9]+\\b"
      },
      {
        "name": "constant.numeric.integer.base.hex.zo",
        "match": "\\bx#[0-9]+\\b"
      },
      {
        "name": "constant.numeric.float.zo",
        "match": "\\b\\d+\\.\\d+([eE][+-]?\\d+)?\\b|\\b\\d+[eE][+-]?\\d+\\b"
      },
      {
        "name": "constant.numeric.integer.decimal.zo",
        "match": "\\b\\d[\\d_]*\\b"
      },
      {
        "name": "constant.language.boolean.zo",
        "match": "\\b(true|false)\\b"
      },
      {
        "name": "string.quoted.single.zo",
        "match": "'([^'\\\\]|\\\\[nrt\\\\'\"]|\\\\x[0-9a-fA-F]{2}|\\\\0)'"
      },
      {
        "name": "string.quoted.double.zo",
        "begin": "\"",
        "end": "\"",
        "patterns": [
          { "include": "#string-escape" },
          { "include": "#string-interpolation" }
        ]
      },
      {
        "name": "string.quoted.raw.zo",
        "begin": "\\$\"",
        "end": "\""
      },
      {
        "name": "constant.other.bytes.zo",
        "match": "`([^`\\\\]|\\\\[nrt\\\\`]|\\\\x[0-9a-fA-F]{2})`"
      }
    ]
  }
}
```

#### Directive Pattern
```json
{
  "name": "meta.preprocessor.directive.zo",
  "match": "(#)(run|dom|if)\\b",
  "captures": {
    "1": { "name": "punctuation.definition.directive.zo" },
    "2": { "name": "keyword.control.directive.zo" }
  }
}
```

#### Typestate Pattern
```json
{
  "name": "meta.typestate.zo",
  "match": "(type@state)\\s+([A-Z][a-zA-Z0-9_]*)",
  "captures": {
    "1": { "name": "keyword.declaration.typestate.zo" },
    "2": { "name": "entity.name.type.zo" }
  }
}
```

#### Nursery Pattern
```json
{
  "name": "meta.nursery.zo",
  "begin": "\\b(nursery)\\s*\\{",
  "beginCaptures": {
    "1": { "name": "keyword.control.concurrency.zo" }
  },
  "end": "\\}",
  "patterns": [
    {
      "match": "\\b(imu)\\s+([a-zA-Z_][a-zA-Z0-9_]*)\\s*(:=)\\s*(spawn|await)",
      "captures": {
        "1": { "name": "keyword.modifier.zo" },
        "2": { "name": "variable.other.zo" },
        "3": { "name": "keyword.operator.assignment.zo" },
        "4": { "name": "keyword.control.concurrency.zo" }
      }
    },
    { "include": "$self" }
  ]
}
```

#### color palette.

| Syntax Category             | Approx. Color (Hex)     | Likely Role / Notes                           |
| --------------------------- | ----------------------- | --------------------------------------------- |
| **Keywords / Operators**    | `#c39ac9` (purple)      | Commonly used for language keywords           |
| **Strings**                 | `#fc9867` (orange)      | Typical for string literals                   |
| **Numbers / Literals**      | `#ffd76d` (yellow)      | Numeric and literal values                    |
| **Functions / Identifiers** | `#bad761` (light green) | Function names, identifiers                   |
| **Types / Classes**         | `#204a87` (blue)        | Types, classes, interface names               |
| **Variables / Properties**  | `#9cd1bb` (teal)        | Variables, object properties                  |
| **Comments / Dim Text**     | `#939293` (grey)        | Comments, less prominent text                 |
| **Punctuation / Syntax**    | `#eaf1f1` (white)       | Brackets, punctuation, general text           |

## 4. Implementation Phases

### Phase 1: Basic Setup (Day 1)
1. Create VS Code extension project structure
2. Set up package.json with language contribution
3. Create language-configuration.json
4. Implement basic comment highlighting
5. Test with simple .zo files

### Phase 2: Core Syntax (Days 2-3)
1. Implement keyword highlighting
2. Add primitive type recognition
3. Implement literal patterns (numbers, strings, booleans)
4. Add operator highlighting
5. Test with overview.zo

### Phase 3: Declarations (Days 4-5)
1. Function declarations with modifiers
2. Struct and enum declarations
3. Type aliases and generics
4. Package and load statements
5. Abstract and apply constructs

### Phase 4: Advanced Features (Days 6-7)
1. Template/ZSX syntax injection
2. Directive highlighting
3. Attribute patterns
4. Typestate syntax
5. Nursery/concurrency keywords

### Phase 5: Polish and Testing (Day 8)
1. String interpolation patterns
2. Raw string literals
3. Closure syntax
4. Pattern matching constructs
5. Comprehensive testing with all samples

### Phase 6: Optimization (Day 9)
1. Performance profiling with large files
2. Regex optimization
3. Scope hierarchy refinement
4. Theme compatibility testing

### Phase 7: Documentation and Release (Day 10)
1. Write comprehensive README
2. Create example screenshots
3. Document all supported features
4. Package and publish to VS Code marketplace

## 5. Testing Strategy

### 5.1 Test Files
Create comprehensive test files covering:
- Basic syntax (variables, functions, types)
- Control flow (if, while, for, loop, match)
- Templates and ZSX
- Directives and attributes
- Typestate and structured concurrency
- Edge cases and complex nesting

### 5.2 Scope Inspector Testing
Use VS Code's Developer: Inspect Editor Tokens and Scopes command to verify:
- Correct scope assignment
- Theme rule application
- Precedence and hierarchy

### 5.3 Theme Compatibility
Test with popular themes:
- Dark+ (default dark)
- Light+ (default light)
- Monokai
- Dracula
- One Dark Pro
- Material Theme

## 6. Advanced Features

### 6.1 Semantic Highlighting (Future)
Prepare for semantic token provider:
```typescript
interface ZoSemanticTokensProvider {
  provideDocumentSemanticTokens(document: TextDocument): SemanticTokens;
  // Distinguish between:
  // - Mutable vs immutable variables
  // - Function calls vs function declarations
  // - Type references vs type declarations
  // - Template expressions vs regular code
}
```

### 6.2 Language Server Integration (Future)
Plan for LSP integration:
- Hover information
- Go to definition
- Find references
- Rename symbol
- Code completion

## 7. Performance Considerations

### 7.1 Regex Optimization
- Avoid backtracking in complex patterns
- Use atomic groups where possible
- Limit lookahead/lookbehind usage
- Profile with large files (10K+ lines)

### 7.2 Grammar Size
- Keep total grammar under 100KB
- Split complex patterns into repository rules
- Use includes strategically to avoid duplication

## 8. Maintenance Plan

### 8.1 Version Control
- Semantic versioning (MAJOR.MINOR.PATCH)
- Changelog for all updates
- Git tags for releases

### 8.2 Issue Tracking
- GitHub issues for bug reports
- Feature request template
- Regular milestone planning

### 8.3 Community Engagement
- Respond to marketplace reviews
- Accept pull requests
- Regular updates based on compiler changes

## 9. Success Metrics

### 9.1 Correctness
- 100% of language constructs highlighted
- No false positives in highlighting
- Proper scope nesting

### 9.2 Performance
- < 50ms tokenization for 1000-line file
- < 200ms for 10,000-line file
- No noticeable lag during typing

### 9.3 User Satisfaction
- 4.5+ star rating on marketplace
- < 5% bug reports per download
- Active community engagement

## 10. Implementation Checklist

- [ ] Project structure created
- [ ] package.json configured
- [ ] language-configuration.json complete
- [ ] Basic grammar structure
- [ ] Comments and documentation
- [ ] Keywords and modifiers
- [ ] Types and generics
- [ ] Literals and strings
- [ ] Operators
- [ ] Function declarations
- [ ] Struct/enum declarations
- [ ] Control flow constructs
- [ ] Template/ZSX syntax
- [ ] Directives
- [ ] Attributes
- [ ] Typestate syntax
- [ ] Concurrency keywords
- [ ] String interpolation
- [ ] Closures and lambdas
- [ ] Pattern matching
- [ ] Test files created
- [ ] Scope inspector validation
- [ ] Theme compatibility verified
- [ ] Performance profiled
- [ ] Documentation written
- [ ] Screenshots captured
- [ ] Extension packaged
- [ ] Marketplace published

## Conclusion

This plan provides a comprehensive roadmap for implementing professional-grade syntax highlighting for the Zo language in VS Code. The phased approach ensures systematic development while maintaining high quality standards. The grammar design accounts for all Zo language features including its unique template system, directives, and typestate capabilities.

Total estimated development time: 10 days for initial release, with ongoing maintenance and feature additions based on user feedback and language evolution.