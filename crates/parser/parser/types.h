#ifndef TYPES_H
#define TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define MAX_INST_ARGS 8
#define MAX_OPERANDS 8
#define MAX_MODIFIERS 4

typedef enum  {
    NoOp,
    LdParamU64,
    LdParamU32,
    LdParamF32,
    MovTidX,
    MovTidY,
    MovTidZ,
    MovCtaidX,
    MovCtaidY,
    MovCtaidZ,
    MovNtidX,
    MovNtidY,
    MovNtidZ,
    MovNctaidX,
    MovNctaidY,
    MovNctaidZ,
    MovU32,
    MovU32Imm,
    MovU64,
    MovU64Imm,
    MovF32,
    MovF32Imm,
    MovF32Bits,
    MovB32FromF32,
    MovF32FromB32,
    NegF32,
    AddS32,
    AddS32Imm,
    AddS64,
    AddF32,
    AddF32Imm,
    SubF32,
    DivRnF32,
    MulF32,
    MulWideS32,
    MadLoS32,
    FmaRnF32,
    FmaRmF32,
    ShlB32,
    RcpRnF32,
    Ex2ApproxF32,
    CvtaToGlobal,
    CvtSatF32F32,
    CvtRnF32S32,
    LdGlobalU32,
    LdGlobalF32,
    LdGlobalNcF32,
    StGlobalU32,
    StGlobalF32,
    SetpGeS32,
    SetpLeF32Imm,
    SetpGeS32Imm,
    SetpLtS32,
    SetpLtS32Imm,
    OrPred,
    Bra,
    BraIf,
    BraIfNot,
    Ret,
    AndB32,
    AndB32Imm,
    SetpEqB32,
    XorPred,
    NotPred,
    ShrU32,
    ShrS32,
    MovPred,
    MadLoS32Imm,
    BraUni,
    SetpEqS32,
    SetpNeS32,
    SetpEqS32Imm,
    SetpNeS32Imm,
    SetpLeF32,
    SetpLtU32,
    SetpLtU32Imm,
    AndPred,
} InstType;


typedef enum {
    REG_BANK_P,
    REG_BANK_R,
    REG_BANK_RD,
    REG_BANK_F } 
RegBank;

typedef enum {
    SREG_TID_X, SREG_TID_Y, SREG_TID_Z,
    SREG_NTID_X, SREG_NTID_Y, SREG_NTID_Z,
    SREG_CTAID_X, SREG_CTAID_Y, SREG_CTAID_Z,
    SREG_NCTAID_X, SREG_NCTAID_Y, SREG_NCTAID_Z,
} SpecialReg;

typedef enum {
    PTX_TYPE_U32, PTX_TYPE_U64, 
    PTX_TYPE_S32, PTX_TYPE_S64,
    PTX_TYPE_F32, 
    PTX_TYPE_B32, PTX_TYPE_B64, 
    PTX_TYPE_PRED,
} PtxType;


typedef enum {
    TOK_COMMA, TOK_SEMICOLON, TOK_COLON,
    TOK_LPAREN, TOK_RPAREN, 
    TOK_LBRACE, TOK_RBRACE,
    TOK_LBRACKET, TOK_RBRACKET,
    TOK_PERCENT, TOK_DOT,
    TOK_AT, TOK_AT_NOT, 
    TOK_LABEL, 
    TOK_FLOAT_BITS, 
    TOK_INT_HEX, TOK_INT_DEC, 
    TOK_ID, TOK_NEWLINE, TOK_EOF,
} TokenType;

typedef struct {
    TokenType type;
    union {
        char* text;
        int64_t int_val;
        uint32_t u32_val; 
    } data;
    
    size_t line;
} Token;

typedef enum { 
    IMM_INT_ZERO, 
    IMM_INT, 
    IMM_F32_BITS
} ImmType;

typedef struct {
    ImmType type;
    union { 
        int64_t int_val; 
        uint32_t f32_bits; 
    } data;
} ImmValue;


typedef enum {
    RAW_OP_REGISTER, RAW_OP_SPECIAL_REG, 
    RAW_OP_IMMEDIATE,
    RAW_OP_MEMORY_REF, 
    RAW_OP_ID, 
    RAW_OP_LABEL,
} RawOperandType;

typedef struct RawOperand RawOperand;
struct RawOperand {
    RawOperandType type;

    union {
        struct { 
            RegBank bank; 
            uint32_t index; 
        } reg;
        SpecialReg special_reg;
        ImmValue imm;
        RawOperand* mem_ref;
        char* id;
        char* label;
    } data;
};

typedef struct {
    uint32_t reg; 
    bool negated; 
} PredGuard;

typedef struct {
    bool has_guard;
    PredGuard guard;
    char* instr_name;
    char* modifiers[MAX_MODIFIERS];
    size_t modifier_count;
    RawOperand operands[MAX_OPERANDS];
    size_t operand_count;
    size_t line;
} RawInstruction;

typedef struct { 
    char* name; 
    PtxType ptx_type;
} ParamInfo;

typedef struct { 
    char* name; 
    PtxType ptx_type; 
    uint8_t pointer_levels;
} SignatureParam;

typedef struct {
    char* name;
    SignatureParam* params;
    size_t param_count;
} ParsedSignature;

typedef struct {
    InstType opcode;
    size_t args[MAX_INST_ARGS];
    size_t arg_count;
} InstInfo;
 
//final output
typedef struct {
    char* name;
    ParamInfo* params;
    size_t param_count;
    InstInfo* instructions;
    size_t instruction_count;
} ParsedKernel;

#endif