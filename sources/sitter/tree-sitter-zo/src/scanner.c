/**
 * External scanner for Zo language.
 * Handles nested block comments: -* ... -* ... *- ... *-
 */

#include "tree_sitter/parser.h"

enum TokenType {
  BLOCK_COMMENT,
};

void *tree_sitter_zo_external_scanner_create(void) {
  return NULL;
}

void tree_sitter_zo_external_scanner_destroy(void *payload) {
  (void)payload;
}

unsigned tree_sitter_zo_external_scanner_serialize(void *payload, char *buffer) {
  (void)payload;
  (void)buffer;
  return 0;
}

void tree_sitter_zo_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  (void)payload;
  (void)buffer;
  (void)length;
}

static void advance(TSLexer *lexer) {
  lexer->advance(lexer, false);
}

bool tree_sitter_zo_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  (void)payload;

  if (valid_symbols[BLOCK_COMMENT]) {
    // Check for -* to start block comment
    if (lexer->lookahead == '-') {
      advance(lexer);
      if (lexer->lookahead == '*') {
        advance(lexer);

        // Track nesting depth
        int depth = 1;

        while (depth > 0 && !lexer->eof(lexer)) {
          if (lexer->lookahead == '-') {
            advance(lexer);
            if (lexer->lookahead == '*') {
              advance(lexer);
              depth++;
            }
          } else if (lexer->lookahead == '*') {
            advance(lexer);
            if (lexer->lookahead == '-') {
              advance(lexer);
              depth--;
            }
          } else {
            advance(lexer);
          }
        }

        lexer->result_symbol = BLOCK_COMMENT;
        return true;
      }
    }
  }

  return false;
}
