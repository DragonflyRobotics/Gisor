#ifndef C_SIGNATURE_H
#define C_SIGNATURE_H

#include "arena.h"
#include "types.h"

Token* tokenize_c(const char* input, Arena* arena, size_t* out_count);
ParsedSignature parse_c_tokens(Token* tokens, size_t token_count, Arena* arena);

#endif