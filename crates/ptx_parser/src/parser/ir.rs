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
    R, //general 32-b int
    Rd, //64-b int 
    F, //32-b float
}

/// PTX special-purpose registers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecialReg {
    //Thread ID within block
    #[default]
    TidX,
    TidY,
    TidZ,
    //Block ID within grid
    CtaidX,
    CtaidY,
    CtaidZ,
    //Block dimensions (threads per block)
    NtidX,
    NtidY,
    NtidZ,
    //Grid dimensions (blocks per grid)
    NctaidX,
    NctaidY,
    NctaidZ,
}

///Literals
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ImmediateValue {
    #[default]
    IntZero,
    Int(i64), //signed or unsigned
    F32Bits(u32),
}

//maps built during lowering

/// Map from PTX label name (e.g. "$L__BB0_2") to instruction index (PC).
/// Built during the first pass over `Vec<RawInstruction>`.
pub type LabelMap = HashMap<String, usize>;

/// Map from PTX parameter identifier (e.g. "_Z9addKernelPfS_S_i_param_0")
/// to its index in `ParsedKernel.params`. Used to resolve `ld.param`
/// operands into arg placeholder indices.
pub type ParamMap = HashMap<String, usize>;