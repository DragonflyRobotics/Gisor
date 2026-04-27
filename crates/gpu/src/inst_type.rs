use crate::execute_unit::ExecuteUnitClass;

#[derive(Default, Clone, Debug)]
pub enum InstType {
    #[default]
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
    SubS32,
    SubS32Imm,
}

impl InstType {
    pub fn execute_unit_class(&self) -> ExecuteUnitClass {
        match self {
            // Memory ops req memory execute units
            InstType::LdParamU64
            | InstType::LdParamU32
            | InstType::LdParamF32
            | InstType::CvtaToGlobal
            | InstType::LdGlobalU32
            | InstType::LdGlobalF32
            | InstType::LdGlobalNcF32
            | InstType::StGlobalU32
            | InstType::StGlobalF32 => ExecuteUnitClass::Memory,

            // special float op
            InstType::Ex2ApproxF32 => ExecuteUnitClass::Special,

            // everything else can run on anything
            _ => ExecuteUnitClass::Generic,
        }
    }
}