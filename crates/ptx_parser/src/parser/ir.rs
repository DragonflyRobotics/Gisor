use std::collections::HashMap;

pub use gpu::inst_info::inst_info;
pub use gpu::inst_type::InstType;

#[derive(Debug, Clone, Default)]
pub struct RawInstruction {
    pub predicate_guard: Option<PredGuard>,
    pub mnemonic: String, ///instr type str
    pub modifiers: Vec<String>,
    pub operands: Vec<RawOperand>,
    pub line: usize, //line num for debugging
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PredGuard {
    pub reg: u32, //predicate reg index
    pub negated: bool,
}

//raw operand info
#[derive(Debug, Clone)]
pub enum RawOperand { //argument types
    Register { bank: RegBank, index: u32 },
    SpecialReg(SpecialReg), //ex: %tid.x
    Immediate(ImmediateValue),
    MemoryRef(Box<RawOperand>), //denoted by bracketed
    Identifier(String),
    Label(String), //branch target (ex: $L__BB0_2), resolved to pc during lowering
}

impl Default for RawOperand {
    fn default() -> Self {
        RawOperand::Immediate(ImmediateValue::default())
    }
}

//type of register
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegBank {
    #[default]
    P, //predicate
    R, //32-b int
    Rd, //64-b int 
    F, //32-b float
}

/// PTX special-purpose registers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecialReg {
    //thread ID within block
    #[default]
    TidX,
    TidY,
    TidZ,
    //block ID within grid
    CtaidX,
    CtaidY,
    CtaidZ,
    //block dimensions (threads per block)
    NtidX,
    NtidY,
    NtidZ,
    //grid dimensions (blocks per grid)
    NctaidX,
    NctaidY,
    NctaidZ,
}

///literals
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ImmediateValue {
    #[default]
    IntZero,
    Int(i64), //signed or unsigned
    F32Bits(u32),
}

//maps built during lowering

///map from PTX label name to instruction index
pub type LabelMap = HashMap<String, usize>; 

///map from PTX parameter identifier to its index in ParsedKernel.params
pub type ParamMap = HashMap<String, usize>;