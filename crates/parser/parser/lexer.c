#include "lexer.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const char* src;
    size_t pos, len, line;
    Arena* arena;
    Token* tokens;
    size_t count, cap;
} Lexer;

static void push_token(Lexer* lex, Token t) {
    if (lex->count == lex->cap) {
        size_t new_cap = lex->cap == 0 ? 64 : lex->cap * 2;
        Token* new_tokens = (Token*)realloc(lex->tokens, new_cap * sizeof(Token));
        if (!new_tokens) {
            fprintf(stderr, "Realloc failed in lexer push token\n"); 
            abort();
        }
        lex->tokens = new_tokens; 
        lex->cap = new_cap;
    }
    lex->tokens[lex->count++] = t;
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

Token* tokenize(const char* input, Arena* arena, size_t* out_count) {
    Lexer lex = {0};
    lex.src = input;
    lex.len = strlen(input);
    lex.line = 1;
    lex.arena = arena;


    while (lex.pos < lex.len) {
        char c = lex.src[lex.pos];

         //skip whitespace
        if (c == ' ' || c == '\t' || c == '\r' || c == '\f') {
            lex.pos++; continue;
        }
        if (c == '\n') {
            push_token(&lex, make_token(TOK_NEWLINE, lex.line));
            lex.line++; lex.pos++; continue;
        }
        //skip comments
        if (c == '/' && lex.pos + 1 < lex.len) {
            if (lex.src[lex.pos+1] == '/') {
                while (lex.pos < lex.len && lex.src[lex.pos] != '\n') lex.pos++;
                continue;
            }
            if (lex.src[lex.pos+1] == '*') {
                lex.pos += 2;
                while (lex.pos + 1 < lex.len) {
                    if (lex.src[lex.pos] == '*' && lex.src[lex.pos+1] == '/') { lex.pos += 2; break; }
                    if (lex.src[lex.pos] == '\n') lex.line++;
                    lex.pos++;
                }
                continue;
            }
        }

        switch (c) {
            case ',': push_token(&lex, make_token(TOK_COMMA, lex.line)); lex.pos++; continue;
            case ';': push_token(&lex, make_token(TOK_SEMICOLON, lex.line)); lex.pos++; continue;
            case ':': push_token(&lex, make_token(TOK_COLON, lex.line)); lex.pos++; continue;
            case '(': push_token(&lex, make_token(TOK_LPAREN, lex.line)); lex.pos++; continue;
            case ')': push_token(&lex, make_token(TOK_RPAREN, lex.line)); lex.pos++; continue;
            case '{': push_token(&lex, make_token(TOK_LBRACE, lex.line)); lex.pos++; continue;
            case '}': push_token(&lex, make_token(TOK_RBRACE, lex.line)); lex.pos++; continue;
            case '[': push_token(&lex, make_token(TOK_LBRACKET, lex.line)); lex.pos++; continue;
            case ']': push_token(&lex, make_token(TOK_RBRACKET, lex.line)); lex.pos++; continue;
            case '%': push_token(&lex, make_token(TOK_PERCENT, lex.line)); lex.pos++; continue;
            case '.': push_token(&lex, make_token(TOK_DOT, lex.line)); lex.pos++; continue;
        }

        if (c == '@') {
            if (lex.pos+1 < lex.len && lex.src[lex.pos+1] == '!') {
                push_token(&lex, make_token(TOK_AT_NOT, lex.line)); lex.pos += 2;
            } else {
                push_token(&lex, make_token(TOK_AT, lex.line)); lex.pos++;
            }
            continue;
        }

        //labels
        if (c == '$') {
            size_t start = lex.pos;
            lex.pos++;
            while (lex.pos < lex.len && is_id_cont(lex.src[lex.pos])) lex.pos++;
            Token t = make_token(TOK_LABEL, lex.line);
            t.data.text = arena_copy(lex.arena, lex.src + start, lex.pos - start);
            push_token(&lex, t);
            continue;
        }

        //number immediates
        if (c >= '0' && c <= '9') {
            size_t start = lex.pos;
            
            //float syntax
            if (c == '0' && lex.pos + 1 < lex.len && (lex.src[lex.pos + 1] == 'f' 
                || lex.src[lex.pos + 1] == 'F')) {
                
                size_t hex_start = lex.pos + 2;
                size_t i = 0;
                while (i < 8 && hex_start + i < lex.len && is_hex(lex.src[hex_start + i])) i++;
                if (i == 8) { //always 8 hex digits
                    char buf[9];
                    memcpy(buf, lex.src + hex_start, 8); buf[8] = '\0';
                    Token t = make_token(TOK_FLOAT_BITS, lex.line);
                    t.data.u32_val = (uint32_t)strtoul(buf, NULL, 16);
                    push_token(&lex, t);
                    lex.pos = hex_start + 8;
                    continue;
                }
            }
            
            //hex int
            if (c == '0' && lex.pos + 1 < lex.len && (lex.src[lex.pos + 1] == 'x' || 
                lex.src[lex.pos + 1] == 'X')) {

                size_t hex_start = lex.pos + 2;
                size_t i = 0;
                while (hex_start + i < lex.len && is_hex(lex.src[hex_start + i])) i++;
                if (i > 0) {
                    char buf[32];
                    size_t n = i < 31 ? i : 31;

                    memcpy(buf, lex.src + hex_start, n);
                    buf[n] = '\0';

                    Token t = make_token(TOK_INT_HEX, lex.line);
                    t.data.int_val = (int64_t)strtoll(buf, NULL, 16);
                    
                    push_token(&lex, t);
                    lex.pos = hex_start + i;
                    continue;
                }
            }
            
            //decimal
            while (lex.pos < lex.len && lex.src[lex.pos] >= '0' && lex.src[lex.pos] <= '9') lex.pos++;
            char buf[32];
            size_t n = lex.pos - start;
            if (n > 31) n = 31;

            memcpy(buf, lex.src + start, n);
            buf[n] = '\0';

            Token t = make_token(TOK_INT_DEC, lex.line);
            t.data.int_val = (int64_t) strtoll(buf, NULL, 10);

            push_token(&lex, t);
            continue;
        }

        if (is_id_start(c)) {
            size_t start = lex.pos;
            while (lex.pos < lex.len && is_id_cont(lex.src[lex.pos])) lex.pos++;
            Token t = make_token(TOK_ID, lex.line);
            t.data.text = arena_copy(lex.arena, lex.src + start, lex.pos - start);
            push_token(&lex, t);
            continue;
        }

        //skip unknown
        lex.pos++;
    }

    push_token(&lex, make_token(TOK_EOF, lex.line));
    *out_count = lex.count;
    return lex.tokens;
}