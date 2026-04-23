//final output types

// ----------------------------------------------------------------------------
// PTX parse output
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ParsedKernel {
    pub name: String, //mangled kernel name

    pub params: Vec<ParamInfo>, //kernel params

    // Fully qualified path to avoid the name collision between the
    // `inst_info` module and the `inst_info` struct inside it.
    pub instructions: Vec<gpu::inst_info::inst_info>, //instr index = pc
}

///kernel param
#[derive(Debug, Clone, Default)]
pub struct ParamInfo {
    pub name: String,       //original param id
    pub ptx_type: PtxType,  //PTX type of param
}

// ----------------------------------------------------------------------------
// C signature parse output
// ----------------------------------------------------------------------------

/// Result of parsing a C/C++ function signature via parse_c_signature().
/// Unlike ParsedKernel, this has no instruction list (a signature has no
/// body) but records pointer-level info per parameter.
#[derive(Debug, Clone, Default)]
pub struct ParsedSignature {
    pub name: String,                    //function name
    pub params: Vec<SignatureParam>,     //params in declaration order
}

/// A single parameter from a C signature.
/// `ptx_type` is the dereferenced type: `float*` gives `F32` with
/// `pointer_levels = 1`. For in-memory size: 8 bytes if pointer_levels > 0,
/// otherwise size is determined by ptx_type.
#[derive(Debug, Clone, Default)]
pub struct SignatureParam {
    pub name: String,           //param name (empty if unnamed)
    pub ptx_type: PtxType,      //dereferenced type
    pub pointer_levels: u8,     //0 for scalar, 1 for T*, 2 for T**, etc.
}

// ----------------------------------------------------------------------------
// Shared: PtxType enum used by both output structs
// ----------------------------------------------------------------------------

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