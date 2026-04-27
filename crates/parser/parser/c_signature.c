#include "c_signature.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const char* src;
    size_t pos, len, line;
    Arena* arena;
    Token* tokens;
    size_t count, cap;
} CLexer;

static void push_token(CLexer* clex, Token t) {
    if (clex->count == clex->cap) {
        size_t new_cap = clex->cap == 0 ? 64 : clex->cap * 2;
        Token* new_tokens = (Token*)realloc(clex->tokens, new_cap * sizeof(Token));
        if (!new_tokens) {
            fprintf(stderr, "Realloc failed in CLexer push token\n"); 
            abort();
        }
        clex->tokens = new_tokens; 
        clex->cap = new_cap;
    }
    clex->tokens[clex->count++] = t;
}

static bool is_id_start(char c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
}
static bool is_id_cont(char c) {
    return is_id_start(c) || (c >= '0' && c <= '9');
}
static bool is_hex(char c) {
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
}

static Token make_token(TokenType type, size_t line) {
    Token t = {0};
    t.type = type;
    t.line = line;
    return t;
}



Token* tokenize_c(const char* input, Arena* arena, size_t* out_count) {
    CLexer clex = {0};
    clex.src = input; 
    clex.len = strlen(input); 
    clex.line = 1; 
    clex.arena = arena;

    while (clex.pos < clex.len) {
        char c = clex.src[clex.pos];

        //skip whitespace
        if (c == ' ' || c == '\t' || c == '\r') {
            clex.pos++;
            continue;
        }
        if (c == '\n') {
            push_token(&clex, make_token(TOK_NEWLINE, clex.line)); 
            clex.line++; 
            clex.pos++; 
            continue;
        }

        //skip comments
        if (c == '/' && clex.pos + 1 < clex.len) {
            if (clex.src[clex.pos+1] == '/') {
                while (clex.pos < clex.len && clex.src[clex.pos] != '\n') clex.pos++;
                continue;
            }
            if (clex.src[clex.pos+1] == '*') {
                clex.pos += 2;
                while (clex.pos + 1 < clex.len) {
                    if (clex.src[clex.pos] == '*' && clex.src[clex.pos+1] == '/') { clex.pos += 2; break; }
                    if (clex.src[clex.pos] == '\n') clex.line++;
                    clex.pos++;
                }
                continue;
            }
        }

        switch (c) {
            case '*': push_token(&clex, make_token(TOK_STAR, clex.line)); clex.pos++; continue;
            case ',': push_token(&clex, make_token(TOK_COMMA, clex.line)); clex.pos++; continue;
            case '(': push_token(&clex, make_token(TOK_LPAREN, clex.line)); clex.pos++; continue;
            case ')': push_token(&clex, make_token(TOK_RPAREN, clex.line)); clex.pos++; continue;
        }

        if (is_id_start(c)) {
            size_t start = clex.pos;
            while (clex.pos < clex.len && is_id_cont(clex.src[clex.pos])) clex.pos++;
            Token t = make_token(TOK_ID, clex.line);
            t.data.text = arena_copy(clex.arena, clex.src + start, clex.pos - start);
            push_token(&clex, t);
            continue;
        }

        clex.pos++; 
    }

    push_token(&clex, make_token(TOK_EOF, clex.line));
    *out_count = clex.count;
    return clex.tokens;
}


typedef struct {
    Token* tokens;
    size_t count;
    size_t cursor;
    size_t line;
    Arena* arena;
} Parser;

static Token* peek(Parser* p) {
    return &p->tokens[p->cursor];
}

static Token* peek_ahead(Parser* p, size_t n) {
    size_t seen = 0;
    for (size_t i = p->cursor; i < p->count; i++) {
        if (p->tokens[i].type == TOK_NEWLINE) continue;
        if (seen == n) return &p->tokens[i];
        seen++;
    }
    return NULL;
}

static void skip_newlines(Parser* p) {
    while (peek(p)->type == TOK_NEWLINE) {
        p->line = peek(p)->line + 1;
        p->cursor++;
    }
}

static Token* advance(Parser* p) {
    Token* t = &p->tokens[p->cursor++];

    if (t->type == TOK_NEWLINE) p->line = t->line + 1;
    else if (t->type != TOK_EOF) p->line = t->line;
    return t;
}

//consumes if matches expecteds
static void expect(Parser* p, TokenType type, const char* expected) {
    skip_newlines(p);
    if (peek(p)->type != type) {
        fprintf(stderr, "Token did not match expected in parser expect");
        abort();
    }
    advance(p);
}

//consumes if its an id
static char* expect_id(Parser* p) {
    skip_newlines(p);
    if (peek(p)->type != TOK_ID){
        fprintf(stderr, "Expected identifier in parser expect_id");
        abort();
    }
    return advance(p)->data.text;
}


static bool parse_c_type_string(const char* s, PtxType* out) {
    //float
    if (!strcmp(s, "float"))  { *out = PTX_TYPE_F32; return true; }
    if (!strcmp(s, "double")) { *out = PTX_TYPE_F32; return true; }

    //32 bit ints
    if (!strcmp(s, "int") || !strcmp(s, "signed int") || !strcmp(s, "signed") || !strcmp(s, "int32_t") || !strcmp(s, "i32"))
        { *out = PTX_TYPE_S32; return true; }
    if (!strcmp(s, "unsigned int") || !strcmp(s, "unsigned") || !strcmp(s, "uint32_t") || !strcmp(s, "u32"))
        { *out = PTX_TYPE_U32; return true; }

    
    //64 bit ints
    if (!strcmp(s, "long") || !strcmp(s, "signed long") || !strcmp(s, "long int") || !strcmp(s, "signed long int")
        || !strcmp(s, "long long") || !strcmp(s, "signed long long") || !strcmp(s, "long long int") || !strcmp(s, "int64_t") || !strcmp(s, "ptrdiff_t"))
        { *out = PTX_TYPE_S64; return true; }
    
    if (!strcmp(s, "unsigned long") || !strcmp(s, "unsigned long int") | !strcmp(s, "unsigned long long") || !strcmp(s, "unsigned long long int")
        || !strcmp(s, "uint64_t") || !strcmp(s, "size_t"))
        { *out = PTX_TYPE_U64; return true; }

    //8-16 bits widen
    if (!strcmp(s, "char") || !strcmp(s, "signed char") || !strcmp(s, "short") || !strcmp(s, "short int")
        || !strcmp(s, "int8_t") || !strcmp(s, "int16_t"))
        { *out = PTX_TYPE_S32; return true; }
    if (!strcmp(s, "unsigned char") || !strcmp(s, "unsigned short") || !strcmp(s, "unsigned short int")
        || !strcmp(s, "uint8_t") || !strcmp(s, "uint16_t"))
        { *out = PTX_TYPE_U32; return true; }

    if (!strcmp(s, "bool") || !strcmp(s, "_Bool")) { 
        *out = PTX_TYPE_PRED; return true;
    }

    return false;
}


static bool is_type_modifier(const char* s) {
    return !strcmp(s, "int") || !strcmp(s, "long") || !strcmp(s, "short") || !strcmp(s, "char") 
        || !strcmp(s, "signed") || !strcmp(s, "unsigned") || !strcmp(s, "struct");
}

static bool is_qualifier(const char* s) {
    return !strcmp(s, "const") || !strcmp(s, "volatile");
}


static char* parse_type_words(Parser* p) {
    skip_newlines(p);

    while (peek(p)->type == TOK_ID && is_qualifier(peek(p)->data.text)) {
        advance(p);
    }

    skip_newlines(p);
    if (peek(p)->type != TOK_ID) {
        fprintf(stderr, "Need type name in c signature\n", p->line);
        abort();
    }

    char buf[256];
    size_t buf_len = 0;

    char* first = advance(p)->data.text;
    size_t fn = strlen(first);
    memcpy(buf, first, fn); buf_len = fn;

   
    for (;;) {
        if (peek(p)->type != TOK_ID) break;
        const char* s = peek(p)->data.text;
        if (!is_type_modifier(s) && !is_qualifier(s)) break;

        advance(p);
        if (is_qualifier(s)) continue;  

        size_t sl = strlen(s);
        buf[buf_len++] = ' ';
        memcpy(buf + buf_len, s, sl); buf_len += sl;
    }

    return arena_copy(p->arena, buf, buf_len);
}

//checks if name has return type
static bool starts_with_bare_name(Parser* p) {
    size_t i = p->cursor;
    while (i < p->count && p->tokens[i].type == TOK_NEWLINE) i++;
    if (i >= p->count || p->tokens[i].type != TOK_ID) return false;
    
    i++;
    while (i < p->count && p->tokens[i].type == TOK_NEWLINE) i++;
    return i < p->count && p->tokens[i].type == TOK_LPAREN;
}


static SignatureParam parse_single_param(Parser* p) {
    SignatureParam out = {0};
    char* type_str = parse_type_words(p);

    //pointer levels
    uint8_t levels = 0;
    while (peek(p)->type == TOK_STAR) { advance(p); if (levels < 255) levels++; }

    //names
    skip_newlines(p);
    char* name = NULL;
    if (peek(p)->type == TOK_ID) name = advance(p)->data.text;
    out.name = name ? name : arena_copy(p->arena, "", 0);

    PtxType type;
    if (!parse_c_type_string(type_str, &type)) {
        fprintf(stderr, "C signature unknown type", p->line, type_str);
        abort();
    }
    out.ptx_type = type;
    out.pointer_levels = levels;
    return out;
}


static SignatureParam* parse_params(Parser* p, size_t* out_count) {
    SignatureParam* arr = NULL;
    size_t n = 0, cap = 0;

    skip_newlines(p);
    if (peek(p)->type == TOK_RPAREN) {
        *out_count = 0;
         return NULL;
    }

    //void special case, no params
    if (peek(p)->type == TOK_ID && !strcmp(peek(p)->data.text, "void")) {
        size_t save = p->cursor;
        advance(p);
        skip_newlines(p);
        if (peek(p)->type == TOK_RPAREN) {
            *out_count = 0; 
            return NULL;
        }
        p->cursor = save;
    }

    for (;;) { //parse params
        skip_newlines(p);
        SignatureParam param = parse_single_param(p);

        //grow array if needed
        if (n == cap) {
            cap = cap ? cap * 2 : 4;
            arr = (SignatureParam*)realloc(arr, cap * sizeof(SignatureParam));
        }
        arr[n++] = param;

        skip_newlines(p);
        if (peek(p)->type == TOK_COMMA) { 
            advance(p); continue; 
        }
        if (peek(p)->type == TOK_RPAREN || peek(p)->type == TOK_EOF) break;
    }

    *out_count = n;
    return arr;
}

ParsedSignature parse_c_tokens(Token* tokens, size_t token_count, Arena* arena) {
    Parser p = {0};
    p.tokens = tokens;
    p.count = token_count;
    p.line = 1;
    p.arena = arena;

    ParsedSignature out = {0};

    skip_newlines(&p);

    char* name;
    if (starts_with_bare_name(&p)) { //demangled
        name = expect_id(&p);
    } else { //has return type
        (void)parse_type_words(&p);
        name = expect_id(&p);
    }
    out.name = name;

    expect(&p, TOK_LPAREN, "`(`");
    out.params = parse_params(&p, &out.param_count);
    expect(&p, TOK_RPAREN, "`)`");

    return out;
}