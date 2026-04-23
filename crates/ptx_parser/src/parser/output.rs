use gpu::inst_info::inst_info;

//final output
#[derive(Debug, Clone, Default)]
pub struct ParsedKernel {
    pub name: String, //mangled kernal name
    pub params: Vec<ParamInfo>, //kernal params
    pub instructions: Vec<inst_info>, //instr index = pc
}

///kernal param
#[derive(Debug, Clone, Default)]
pub struct ParamInfo {
    pub name: String, //original param id
    pub ptx_type: PtxType, //PTX type of param
}

/// PTX scalar types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PtxType {
    #[default]
    U32,
    U64,
    S32,
    S64,
    F32,
    B32,
    B64,
    Pred,
}
