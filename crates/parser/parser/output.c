#include "output.h"
#include "arena.h"
#include "lexer.h"
#include "lowering.h"
#include "parser.h"
#include "c_signature.h"
#include <stdlib.h>
#include <string.h>

static char* dup_string(const char* s) {
    size_t n = strlen(s) + 1;
    char* p = (char*)malloc(n);
    memcpy(p, s, n);
    return p;
}

ParsedKernel ptx_parse(const char* source) {
    Arena arena = arena_create(1 << 20);  

    size_t token_count = 0;
    Token* tokens = tokenize(source, &arena, &token_count);

    ParseOutput po = parse_tokens(tokens, token_count, &arena);
    ParsedKernel k = lower(po, &arena);

    k.name = dup_string(k.name);
    for (size_t i = 0; i < k.param_count; i++) {
        k.params[i].name = dup_string(k.params[i].name);
    }

    free(tokens);
    free(po.raw_instructions);
    arena_destroy(&arena);

    return k;
}

void ptx_parse_destroy(ParsedKernel* k) {
    if (!k) return;

    free(k->name);
    for (size_t i = 0; i < k->param_count; i++) {
        free(k->params[i].name);
    }
    free(k->params);
    free(k->instructions);

    k->name = NULL;
    k->params = NULL;
    k->instructions = NULL;
    k->param_count = 0;
    k->instruction_count = 0;
}


ParsedSignature parse_c_signature(const char* source) {
    Arena arena = arena_create(1 << 16);

    size_t token_count = 0;
    Token* tokens = tokenize_c(source, &arena, &token_count);

    ParsedSignature sig = parse_c_tokens(tokens, token_count, &arena);
    
    sig.name = dup_string(sig.name);
    for (size_t i = 0; i < sig.param_count; i++) {
        sig.params[i].name = dup_string(sig.params[i].name);
    }

    free(tokens);
    arena_destroy(&arena);

    return sig;
}

void parse_c_signature_destroy(ParsedSignature* sig) {
    if (!sig) return;
    free(sig->name);
    
    for (size_t i = 0; i < sig->param_count; i++) {
        free(sig->params[i].name);
    }
    free(sig->params);

    sig->name = NULL;
    sig->params = NULL;
    sig->param_count = 0;
}