// pub mod parser;
// pub use parser::{parse, parse_c_signature, ParseError, ParsedKernel, ParamInfo, PtxType, inst_info, InstType};

use libc::{c_char, int64_t, uint32_t};
use gpu::inst_type::InstType;

const MAX_INST_ARGS: usize = 8;
const MAX_OPERANDS: usize  = 8;
const MAX_MODIFIERS: usize = 4;
#[repr(C)]
enum RegBank {
    REG_BANK_P,
    REG_BANK_R,
    REG_BANK_RD,
    REG_BANK_F,
}

#[repr(C)]
enum SpecialReg {
    SREG_TID_X,
    SREG_TID_Y,
    SREG_TID_Z,
    SREG_NTID_X,
    SREG_NTID_Y,
    SREG_NTID_Z,
    SREG_CTAID_X,
    SREG_CTAID_Y,
    SREG_CTAID_Z,
    SREG_NCTAID_X,
    SREG_NCTAID_Y,
    SREG_NCTAID_Z,
}

#[repr(C)]
enum PtxType {
    PTX_TYPE_U32,
    PTX_TYPE_U64,
    PTX_TYPE_S32,
    PTX_TYPE_S64,
    PTX_TYPE_F32,
    PTX_TYPE_B32,
    PTX_TYPE_B64,
    PTX_TYPE_PRED,
}

#[repr(C)]
enum TokenType {
    TOK_COMMA,
    TOK_SEMICOLON,
    TOK_COLON,
    TOK_LPAREN,
    TOK_RPAREN,
    TOK_LBRACE,
    TOK_RBRACE,
    TOK_LBRACKET,
    TOK_RBRACKET,
    TOK_PERCENT,
    TOK_DOT,
    TOK_AT,
    TOK_AT_NOT,
    TOK_LABEL,
    TOK_FLOAT_BITS,
    TOK_INT_HEX,
    TOK_INT_DEC,
    TOK_ID,
    TOK_NEWLINE,
    TOK_EOF,
}

#[repr(C)]
union TokenData {
    text: *const c_char,
    int_val: i64,
    u32_val: u32,
}

#[repr(C)]
struct Token {
    token_type: TokenType,
    data: TokenData,
    line: usize,
}

#[repr(C)]
enum ImmType {
    IMM_INT_ZERO,
    IMM_INT,
    IMM_F32_BITS,
}

#[repr(C)]
union ImmData {
    text: *const c_char,
    int_val: i64,
    u32_val: u32,
}

#[repr(C)]
struct ImmValue {
    imm_type: ImmType,
    imm_data: ImmData,
}

#[repr(C)]
enum RawOperandType {
    RAW_OP_REGISTER,
    RAW_OP_SPECIAL_REG,
    RAW_OP_IMMEDIATE,
    RAW_OP_MEMORY_REF,
    RAW_OP_ID,
    RAW_OP_LABEL,
}

#[repr(C)]
struct reg {
    bank: RegBank,
    index: u32,
}

#[repr(C)]
union RawOperandData {
    reg: std::mem::ManuallyDrop<reg>,
    special_reg: std::mem::ManuallyDrop<SpecialReg>,
    imm: std::mem::ManuallyDrop<ImmValue>,
    mem_ref: *const RawOperandData,
    id: *const c_char,
    label: *const c_char,
}

#[repr(C)]
struct RawOperand {
    raw_operand_type: RawOperandType,
    raw_operand_data: RawOperandData,
}

#[repr(C)]
struct PredGuard {
    reg: u32,
    negated: bool,
}

#[repr(C)]
struct RawInstruction {
    has_guard: bool,
    guard: PredGuard,
    instr_name: *const c_char,
    modifiers: [*const c_char; MAX_MODIFIERS],
    modifier_count: usize,
    operands: [*const RawOperand; MAX_OPERANDS],
    operand_count: usize,
    line: usize,
}

#[repr(C)]
struct ParamInfo {
    name: *const c_char,
    ptx_type: PtxType,
}

#[repr(C)]
struct SignatureParam {
    name: *const c_char,
    ptx_type: PtxType,
    pointer_levels: u8,
}

#[repr(C)]
struct InstInfo {
    opcode: InstType,
    args: [usize; MAX_INST_ARGS],
    arg_count: usize,
}

#[repr(C)]
struct ParsedKernel {
    name: *const c_char,
    params: *const ParamInfo,
    param_count: usize,
    instructions: *const InstInfo,
    instruction_count: usize,
}