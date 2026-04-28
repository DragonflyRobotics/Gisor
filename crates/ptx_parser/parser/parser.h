#ifndef PARSER_H
#define PARSER_H

#include "arena.h"
#include "types.h"

typedef struct {
    char* name;
    ParamInfo* params;
    size_t param_count;
    RawInstruction* raw_instructions;
    size_t raw_count;
} ParseOutput;

ParseOutput parse_tokens(Token* tokens, size_t token_count, Arena* arena);

#endif