#ifndef LEXER_H
#define LEXER_H

#include "arena.h"
#include "types.h"

Token* tokenize(const char* input, Arena* arena, size_t* out_count);

#endif