#include "parser.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

//consumes if id matches name
static bool consume_id(Parser* p, const char* name) {
    skip_newlines(p);
    if (peek(p)->type == TOK_ID && strcmp(peek(p)->data.text, name) == 0) {
        advance(p);
        return true;
    }
    return false;
}

static PtxType parse_ptx_type(Parser* p, const char* s) {
    if (!strcmp(s, "u32")) return PTX_TYPE_U32;
    if (!strcmp(s, "u64")) return PTX_TYPE_U64;
    if (!strcmp(s, "s32")) return PTX_TYPE_S32;
    if (!strcmp(s, "s64")) return PTX_TYPE_S64;
    if (!strcmp(s, "f32")) return PTX_TYPE_F32;
    if (!strcmp(s, "b32")) return PTX_TYPE_B32;
    if (!strcmp(s, "b64")) return PTX_TYPE_B64;
    if (!strcmp(s, "pred")) return PTX_TYPE_PRED;
    return 0;
}

static void parse_register_name(Parser* p, const char* s, RegBank* out_bank, uint32_t* out_idx) {
    size_t i = 0;
    while (s[i] && !(s[i] >= '0' && s[i] <= '9')) i++;

    if (i == 1 && s[0] == 'p') *out_bank = REG_BANK_P;
    else if (i == 1 && s[0] == 'r') *out_bank = REG_BANK_R;
    else if (i == 2 && s[0] == 'r' && s[1] == 'd') *out_bank = REG_BANK_RD;
    else if (i == 1 && s[0] == 'f') *out_bank = REG_BANK_F;
    else {
        fprintf(stderr, "Unexpected register bank prefix"); 
        abort();
    }
    
    *out_idx = (uint32_t)strtoul(s + i, NULL, 10);
    if (*out_idx > 255) {
        fprintf(stderr, "Register index is out of bounds"); 
        abort();
    }
}

static uint32_t parse_pred_index(Parser* p, const char* s) {
    RegBank b;
    uint32_t i;
    parse_register_name(p, s, &b, &i);
    if (b != REG_BANK_P) {
        fprintf(stderr, "Not a predicate register in parse_pred_index"); 
        abort();
    }
    return i;
}


//OPERAND PARSING
static RawOperand parse_operand(Parser* p);

static bool is_special_reg(const char* s) {
    return !strcmp(s,"tid") || !strcmp(s,"ntid") || !strcmp(s,"ctaid") || !strcmp(s,"nctaid");
}

static bool match_special_reg(const char* name, const char* axis, SpecialReg* out) {
    if (strcmp(name, "tid") == 0) {
        if (strcmp(axis, "x") == 0) { *out = SREG_TID_X; return true; }
        if (strcmp(axis, "y") == 0) { *out = SREG_TID_Y; return true; }
        if (strcmp(axis, "z") == 0) { *out = SREG_TID_Z; return true; }
    }
    if (strcmp(name, "ntid") == 0) {
        if (strcmp(axis, "x") == 0) { *out = SREG_NTID_X; return true; }
        if (strcmp(axis, "y") == 0) { *out = SREG_NTID_Y; return true; }
        if (strcmp(axis, "z") == 0) { *out = SREG_NTID_Z; return true; }
    }
    if (strcmp(name, "ctaid") == 0) {
        if (strcmp(axis, "x") == 0) { *out = SREG_CTAID_X; return true; }
        if (strcmp(axis, "y") == 0) { *out = SREG_CTAID_Y; return true; }
        if (strcmp(axis, "z") == 0) { *out = SREG_CTAID_Z; return true; }
    }
    if (strcmp(name, "nctaid") == 0) {
        if (strcmp(axis, "x") == 0) { *out = SREG_NCTAID_X; return true; }
        if (strcmp(axis, "y") == 0) { *out = SREG_NCTAID_Y; return true; }
        if (strcmp(axis, "z") == 0) { *out = SREG_NCTAID_Z; return true; }
    }
    return false;
}

static RawOperand parse_operand(Parser* p) {
    skip_newlines(p);
    RawOperand op = {0};
    Token* t = peek(p);

    //type of operand to parse
    switch (t->type) {
        case TOK_PERCENT: { //registers
            advance(p);
            char* name = expect_id(p);
            if (is_special_reg(name) && peek(p)->type == TOK_DOT) {
                advance(p);
                char* axis = expect_id(p);
                SpecialReg sr;
                if (!match_special_reg(name, axis, &sr)) {
                    fprintf(stderr, "Did not match special register names"); 
                    abort();
                }

                op.type = RAW_OP_SPECIAL_REG;
                op.data.special_reg = sr;
                return op;
            }
            
            op.type = RAW_OP_REGISTER;
            parse_register_name(p, name, &op.data.reg.bank, &op.data.reg.index);
            return op;
        }
        case TOK_LBRACKET: {
            advance(p);
            RawOperand inner = parse_operand(p);
            expect(p, TOK_RBRACKET, "`]`");
            op.type = RAW_OP_MEMORY_REF;
            op.data.mem_ref = (RawOperand*) arena_alloc(p->arena, sizeof(RawOperand));
            *op.data.mem_ref = inner;
            return op;
        }
        case TOK_LABEL:
            op.type = RAW_OP_LABEL;
            op.data.label = advance(p)->data.text;
            return op;
        case TOK_INT_DEC:
        case TOK_INT_HEX: {
            Token* num = advance(p);
            op.type = RAW_OP_IMMEDIATE;
            op.data.imm.type = IMM_INT;
            op.data.imm.data.int_val = num->data.int_val;
            return op;
        }
        case TOK_FLOAT_BITS: {
            Token* num = advance(p);
            op.type = RAW_OP_IMMEDIATE;
            op.data.imm.type = IMM_F32_BITS;
            op.data.imm.data.f32_bits = num->data.u32_val;
            return op;
        }
        case TOK_ID:
            op.type = RAW_OP_ID;
            op.data.id = advance(p)->data.text;
            return op;
        default:
            fprintf(stderr, "Unknown operand");
            abort();
            return op;
    }
}

static void parse_instruction(Parser* p, RawInstruction* out) {
    skip_newlines(p);
    memset(out, 0, sizeof(*out));
    out->line = p->line;
    
    //predicate guard
    if (peek(p)->type == TOK_AT || peek(p)->type == TOK_AT_NOT) { 
        TokenType type = peek(p)->type;
        advance(p);
        expect(p, TOK_PERCENT, "`%`");
        out->has_guard = true;
        out->guard.reg = parse_pred_index(p, expect_id(p));        
        out->guard.negated = type == TOK_AT ? false : true;
    }

 
    out->instr_name = expect_id(p);
    
    //modifiers
    while (peek(p)->type == TOK_DOT) {
        advance(p);

        if (out->modifier_count >= MAX_MODIFIERS) {
            fprintf(stderr, "Number of modifiers goes over maximum amount"); 
            abort();
        }
        out->modifiers[out->modifier_count++] = expect_id(p);
    }
    
    //operands
    for (;;) {
        skip_newlines(p);
        Token* t = peek(p);
        if (t->type == TOK_SEMICOLON) { advance(p); break; }
        if (t->type == TOK_COMMA) { advance(p); continue; }

        if (t->type == TOK_EOF) {
            fprintf(stderr, "Unexpected way to end an instruction"); 
            abort();
        }
        if (out->operand_count >= MAX_OPERANDS) {
            fprintf(stderr, "Number of operands for instruction is more than the max"); 
            abort();
        }
        out->operands[out->operand_count++] = parse_operand(p);
    }
}




static void skip_header_directives(Parser* p) {
    for (;;) {
        skip_newlines(p);
        Token* t = peek(p);
        if (t->type == TOK_EOF) {
            fprintf(stderr, "Kernel entry directive is expected"); abort();
        }
        if (t->type != TOK_DOT) {
            fprintf(stderr, "Expected file-level directive"); abort();
        }
        Token* name = peek_ahead(p, 1);
        if (!name || name->type != TOK_ID) {
            fprintf(stderr, "Expected directive name after `.`"); abort();
        }
        if (!strcmp(name->data.text, "visible") || !strcmp(name->data.text, "entry")) return;
        while (peek(p)->type != TOK_NEWLINE && peek(p)->type != TOK_EOF) advance(p);
    }
}


static void parse_entry_header(Parser* p, ParseOutput* out) {
    expect(p, TOK_DOT, "`.` before entry directive");
    if (consume_id(p, "visible")) {
        expect(p, TOK_DOT, "`.` before `entry`");
    }
    if (!consume_id(p, "entry")) { fprintf(stderr, "Expected `entry`"); abort(); }
    out->name = expect_id(p);

    expect(p, TOK_LPAREN, "`(` before parameter list");

    size_t cap = 0;
    for (;;) {
        skip_newlines(p);
        Token* t = peek(p);
        if (t->type == TOK_RPAREN) break;
        if (t->type == TOK_COMMA)  { advance(p); continue; }
        if (t->type != TOK_DOT)    { fprintf(stderr, "Expected `.param` or `)`"); abort(); }

        advance(p);
        if (!consume_id(p, "param")) { fprintf(stderr, "Expected `param`"); abort(); }
        expect(p, TOK_DOT, "`.` before param type");
        char* ty_str = expect_id(p);
        PtxType ty = parse_ptx_type(p, ty_str);
        char* pname = expect_id(p);

        if (out->param_count == cap) {
            cap = cap ? cap * 2 : 4;
            out->params = (ParamInfo*)realloc(out->params, cap * sizeof(ParamInfo));
        }
        out->params[out->param_count].name = pname;
        out->params[out->param_count].ptx_type = ty;
        out->param_count++;
    }
    expect(p, TOK_RPAREN, "`)` after parameter list");
}

static void parse_reg_decls(Parser* p) {
    for (;;) {
        skip_newlines(p);
        if (peek(p)->type != TOK_DOT) break;
        Token* next = peek_ahead(p, 1);
        if (!next || next->type != TOK_ID || strcmp(next->data.text, "reg") != 0) break;

        advance(p); advance(p); 
        expect(p, TOK_DOT, "`.` before reg type");
        (void)expect_id(p); 
        expect(p, TOK_PERCENT, "`%` before register bank");
        char* bank = expect_id(p);
        if (peek(p)->type == TOK_INT_DEC) advance(p);
        
        expect(p, TOK_SEMICOLON, "`;`");
        
        if (strcmp(bank, "p") && strcmp(bank, "r") && strcmp(bank, "rd") && strcmp(bank, "f")) {
            fprintf(stderr, "Unknown register bank"); abort();
        }
    }
}

static void parse_instruction_list(Parser* p, ParseOutput* out) {
    size_t cap = 0;
    for (;;) {
        skip_newlines(p);
        Token* t = peek(p);
        if (t->type == TOK_RBRACE || t->type == TOK_EOF) break;

        if (out->raw_count == cap) {
            cap = cap ? cap * 2 : 32;
            out->raw_instructions = (RawInstruction*)realloc(
                out->raw_instructions, cap * sizeof(RawInstruction));
        }

        RawInstruction* inst = &out->raw_instructions[out->raw_count];
        if (t->type == TOK_LABEL) {
            char* lbl = advance(p)->data.text;
            expect(p, TOK_COLON, "`:` after label");
            memset(inst, 0, sizeof(*inst));
            inst->instr_name = arena_copy(p->arena, ".label", 6);
            inst->line = p->line;
            inst->operand_count = 1;
            inst->operands[0].type = RAW_OP_LABEL;
            inst->operands[0].data.label = lbl;
        } else {
            parse_instruction(p, inst);
        }
        out->raw_count++;
    }
}

ParseOutput parse_tokens(Token* tokens, size_t token_count, Arena* arena) {
    Parser p = {0};
    p.tokens = tokens;
    p.count = token_count;
    p.line = 1;
    p.arena = arena;

    ParseOutput out = {0};
    skip_header_directives(&p);
    parse_entry_header(&p, &out);
    expect(&p, TOK_LBRACE, "`{` to start kernel body");
    parse_reg_decls(&p);
    parse_instruction_list(&p, &out);
    expect(&p, TOK_RBRACE, "`}` to close kernel body");
    return out;
}