use std::{ffi::{CString, c_char}, ops::Add};

use gpu::{inst_info::inst_info, inst_type::InstType};

const MAX_INST_ARGS: usize = 8;
const MAX_OPERANDS: usize  = 8;
const MAX_MODIFIERS: usize = 4;

#[repr(C)]
pub enum RegBank {
    REG_BANK_P,
    REG_BANK_R,
    REG_BANK_RD,
    REG_BANK_F,
}

#[repr(C)]
pub enum SpecialReg {
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
#[derive(Debug, Clone, Copy)]
pub enum PtxType {
    U32,
    U64,
    S32,
    S64,
    F32,
    B32,
    B64,
    Pred,
}

#[repr(C)]
pub enum TokenType {
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
pub union TokenData {
    text: *const c_char,
    int_val: i64,
    u32_val: u32,
}

#[repr(C)]
pub struct Token {
    token_type: TokenType,
    data: TokenData,
    line: usize,
}

#[repr(C)]
pub enum ImmType {
    IMM_INT_ZERO,
    IMM_INT,
    IMM_F32_BITS,
}

#[repr(C)]
pub union ImmData {
    text: *const c_char,
    int_val: i64,
    u32_val: u32,
}

#[repr(C)]
pub struct ImmValue {
    imm_type: ImmType,
    imm_data: ImmData,
}

#[repr(C)]
pub enum RawOperandType {
    RAW_OP_REGISTER,
    RAW_OP_SPECIAL_REG,
    RAW_OP_IMMEDIATE,
    RAW_OP_MEMORY_REF,
    RAW_OP_ID,
    RAW_OP_LABEL,
}

#[repr(C)]
pub struct reg {
    bank: RegBank,
    index: u32,
}

#[repr(C)]
pub union RawOperandData {
    reg: std::mem::ManuallyDrop<reg>,
    special_reg: std::mem::ManuallyDrop<SpecialReg>,
    imm: std::mem::ManuallyDrop<ImmValue>,
    mem_ref: *const RawOperandData,
    id: *const c_char,
    label: *const c_char,
}

#[repr(C)]
pub struct RawOperand {
    raw_operand_type: RawOperandType,
    raw_operand_data: RawOperandData,
}

#[repr(C)]
pub struct PredGuard {
    reg: u32,
    negated: bool,
}

#[repr(C)]
pub struct RawInstruction {
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
pub struct ParamInfo {
    name: *const c_char,
    ptx_type: PtxType,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignatureParam {
    name: *const c_char,
    pub ptx_type: PtxType,
    pub pointer_levels: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InstInfo {
    opcode: InstType,
    args: [usize; MAX_INST_ARGS],
    arg_count: usize,
}

#[repr(C)]
pub struct ParsedKernel {
    name: *const c_char,
    params: *const ParamInfo,
    param_count: usize,
    instructions: *const InstInfo,
    instruction_count: usize,
}

#[repr(C)]
pub struct ParsedSignature {
    name: *const c_char,
    params: *const SignatureParam,
    param_count: usize,
}

pub struct ParsedSignatureRust {
    pub name: String,
    pub params: Vec<SignatureParam>,
}

unsafe extern "C" {
    pub fn ptx_parse(source: *const c_char) -> ParsedKernel;
    fn parse_c_signature(source: *const c_char) -> ParsedSignature;
}

pub fn translate(parsed: ParsedKernel) -> Vec<inst_info> {
    let mut inst: Vec<inst_info> = Vec::new();
    unsafe {
        for i in 0..parsed.instruction_count {
            let c = *(parsed.instructions.add(i));
            let args = c.args[..c.arg_count].to_vec();
            let rust = inst_info {
                inst_type: c.opcode,
                args: args,
            };
            inst.push(rust);
        }
    }
    inst
}

pub fn parse_rust_signature(name: &str) -> ParsedSignatureRust {
    unsafe {
        let parsed_c = parse_c_signature(CString::new(name).unwrap().as_ptr());
        let mut signatures: Vec<SignatureParam> = Vec::new();
        for i in 0..parsed_c.param_count {
            signatures.push(*parsed_c.params.add(i));
        }
        let name = CString::from_raw(parsed_c.name as *mut i8);
        ParsedSignatureRust {
            name: String::from_utf8_lossy(name.as_bytes()).to_string(),
            params: signatures,
        }
    }
}

pub fn parse(source: &str) -> Vec<inst_info> {
    let c_str = CString::new(source).unwrap();
    let parsed = unsafe { ptx_parse(c_str.as_ptr()) };
    let res = translate(parsed);
    // for ins in &res {
    //     println!("Inst: {:?}", ins);
    // }
    // panic!("KYS");
    res
}
