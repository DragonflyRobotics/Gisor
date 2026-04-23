//! convert ParseOutput into the final `ParsedKernel`.
//!   - pick the right `InstType` variant based on (mnemonic, modifiers,
//!     operand shape)
//!   - rewrite special cases like `mov.u32 %r, %tid.x` -> `MovTidX`
//!   - resolve `$label` operands to PC values
//!   - resolve `[param_name]` operands to parameter placeholder indices
//!   - encode each operand into a `usize` slot for Zekai's `inst_info.args`


use std::collections::HashMap;

use crate::parser::ir::{
    inst_info, ImmediateValue, InstType, LabelMap, ParamMap, PredGuard, RawInstruction, RawOperand,
    RegBank, SpecialReg,
};
use crate::parser::error::ParseError;
use crate::parser::output::ParsedKernel;
use crate::parser::parser::ParseOutput;
use gpu::inst_info::make_inst;

pub fn lower(parse_out: ParseOutput) -> Result<ParsedKernel, ParseError> {
    //build the param name -> index map
    let mut param_map: ParamMap = HashMap::new();
    for (i, p) in parse_out.params.iter().enumerate() {
        param_map.insert(p.name.clone(), i);
    }

    // Pass 1: build the label map by walking the raw instructions and counting only real ones.
    let label_map = build_label_map(&parse_out.raw_instructions);

    // Pass 2: emit inst_info for each real instruction.
    let mut instructions = Vec::new();
    for raw in &parse_out.raw_instructions {
        if raw.mnemonic == ".label" {
            continue; // pseudo-instructions are not emitted
        }
        let lowered = lower_instruction(raw, &label_map, &param_map)?;
        instructions.push(lowered);
    }

    Ok(ParsedKernel {
        name: parse_out.name,
        params: parse_out.params,
        instructions,
    })
}


/// build label_name -> pc map. 
fn build_label_map(raw: &[RawInstruction]) -> LabelMap {
    let mut map = LabelMap::new();
    let mut pc: usize = 0;
    for inst in raw {
        if inst.mnemonic == ".label" {
            //label points at the NEXT real instruction's PC
            if let Some(RawOperand::Label(s)) = inst.operands.first() {
                map.insert(s.clone(), pc);
            }
        } else {
            pc += 1;
        }
    }
    map
}

fn lower_instruction(
    raw: &RawInstruction,
    labels: &LabelMap,
    params: &ParamMap,
) -> Result<inst_info, ParseError> {
    //predicate guards only supported on branch instrs
    if let Some(guard) = raw.predicate_guard {
        if raw.mnemonic != "bra" {
            return Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "predicate guards are only supported on `bra` instructions".to_string(),
            });
        }
        return lower_branch(raw, Some(guard), labels);
    }

    // Dispatch on mnemonic.
    match raw.mnemonic.as_str() {
        "ld" => lower_ld(raw, params),
        "st" => lower_st(raw),
        "mov" => lower_mov(raw),
        "mad" => lower_mad(raw),
        "fma" => lower_fma(raw),
        "add" => lower_add(raw),
        "sub" => lower_sub(raw),
        "mul" => lower_mul(raw),
        "neg" => lower_neg(raw),
        "setp" => lower_setp(raw),
        "or" => lower_or(raw),
        "shl" => lower_shl(raw),
        "cvta" => lower_cvta(raw),
        "cvt" => lower_cvt(raw),
        "ex2" => lower_ex2(raw),
        "rcp" => lower_rcp(raw),
        "bra" => lower_branch(raw, None, labels),
        "ret" => Ok(make_inst(InstType::Ret, vec![])),
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}


//opcode specific lowering helpers

/// `ld.param.u64 %rd1, [param_name]` or `ld.global.f32 %f1, [%rd8]`.
fn lower_ld(raw: &RawInstruction, params: &ParamMap) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    let dest = reg_index(&raw.operands[0], raw.line)?;

    match raw.modifiers.as_slice() {
        // ld.param.u64 %rd, [param_name]
        [m, ty] if m == "param" && ty == "u64" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamU64, vec![dest, arg_idx]))
        }
        [m, ty] if m == "param" && ty == "u32" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamU32, vec![dest, arg_idx]))
        }
        [m, ty] if m == "param" && ty == "f32" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamF32, vec![dest, arg_idx]))
        }
        // ld.global.f32 %f, [%rd]
        [m, ty] if m == "global" && ty == "f32" => {
            let addr = memref_reg(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::LdGlobalF32, vec![dest, addr]))
        }
        // ld.global.nc.f32 %f, [%rd]  -- non-coherent cached load; same
        // semantics for our emulator but we preserve the opcode so Zekai
        // can instrument it separately if he wants.
        [m1, m2, ty] if m1 == "global" && m2 == "nc" && ty == "f32" => {
            let addr = memref_reg(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::LdGlobalNcF32, vec![dest, addr]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `st.global.f32 [%rd], %f` — note address is args[0], value is args[1].
fn lower_st(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m, ty] if m == "global" && ty == "f32" => {
            let addr = memref_reg(&raw.operands[0], raw.line)?;
            let value = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::StGlobalF32, vec![addr, value]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

///   mov.u32 %r, %tid.x      -> MovTidX [r]          (special register read)
///   mov.u32 %r, %ntid.y     -> MovNtidY [r]
///   mov.u32 %r, %ctaid.x    -> MovCtaidX [r]
///   mov.u32 %r, %nctaid.x   -> MovNctaidX [r]
///   mov.u32 %r, %r2         -> MovU32 [r, r2]            (reg-to-reg)
///   mov.u32 %r, <imm>       -> MovU32Imm [r, imm]
///   mov.u64 %rd, %rd2       -> MovU64 [rd, rd2]
///   mov.u64 %rd, <imm>      -> MovU64Imm [rd, imm]
///   mov.f32 %f, 0f...       -> MovF32Imm [f, bits]
///   mov.f32 %f, %f          -> MovF32 [dest, src]
///   mov.b32 %r, %f          -> MovB32FromF32 [dest, src]   (bitcast)
///   mov.f32 %f, %r          -> MovF32FromB32 [dest, src]   (bitcast)
fn lower_mov(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    let dest_op = &raw.operands[0];
    let src_op = &raw.operands[1];

    // Special register sources: these override everything else.
    if let RawOperand::SpecialReg(sr) = src_op {
        let dest = reg_index(dest_op, raw.line)?;
        let opcode = match sr {
            SpecialReg::TidX => InstType::MovTidX,
            SpecialReg::TidY => InstType::MovTidY,
            SpecialReg::TidZ => InstType::MovTidZ,
            SpecialReg::CtaidX => InstType::MovCtaidX,
            SpecialReg::CtaidY => InstType::MovCtaidY,
            SpecialReg::CtaidZ => InstType::MovCtaidZ,
            SpecialReg::NtidX => InstType::MovNtidX,
            SpecialReg::NtidY => InstType::MovNtidY,
            SpecialReg::NtidZ => InstType::MovNtidZ,
            SpecialReg::NctaidX => InstType::MovNctaidX,
            SpecialReg::NctaidY => InstType::MovNctaidY,
            SpecialReg::NctaidZ => InstType::MovNctaidZ,
        };
        return Ok(make_inst(opcode, vec![dest]));
    }

    // Dispatch by type modifier (u32, u64, f32, b32) and source kind.
    match raw.modifiers.as_slice() {
        [ty] if ty == "u32" => {
            let dest = reg_index(dest_op, raw.line)?;
            match src_op {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::MovU32Imm,
                    vec![dest, imm_to_usize(*imm)],
                )),
                RawOperand::Register { bank: RegBank::R, index } => {
                    Ok(make_inst(InstType::MovU32, vec![dest, *index as usize]))
                }
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "mov.u32 source must be an r-register or immediate".to_string(),
                }),
            }
        }
        [ty] if ty == "u64" => {
            let dest = reg_index(dest_op, raw.line)?;
            match src_op {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::MovU64Imm,
                    vec![dest, imm_to_usize(*imm)],
                )),
                RawOperand::Register { bank: RegBank::Rd, index } => {
                    Ok(make_inst(InstType::MovU64, vec![dest, *index as usize]))
                }
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "mov.u64 source must be an rd-register or immediate".to_string(),
                }),
            }
        }
        [ty] if ty == "f32" => {
            let dest = reg_index(dest_op, raw.line)?;
            match src_op {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::MovF32Imm,
                    vec![dest, imm_to_usize(*imm)],
                )),
                RawOperand::Register { bank: RegBank::F, index } => {
                    Ok(make_inst(InstType::MovF32, vec![dest, *index as usize]))
                }
                RawOperand::Register { bank: RegBank::R, index } => {
                    // mov.f32 %f, %r -> reinterpret integer bits as float
                    Ok(make_inst(
                        InstType::MovF32FromB32,
                        vec![dest, *index as usize],
                    ))
                }
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "unsupported source operand for mov.f32".to_string(),
                }),
            }
        }
        [ty] if ty == "b32" => {
            // mov.b32 %r, %f -> MovB32FromF32
            let dest = reg_index(dest_op, raw.line)?;
            match src_op {
                RawOperand::Register { bank: RegBank::F, index } => Ok(make_inst(
                    InstType::MovB32FromF32,
                    vec![dest, *index as usize],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "unsupported source operand for mov.b32".to_string(),
                }),
            }
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `mad.lo.s32 %dest, %a, %b, %c` -> MadLoS32 [dest, a, b, c].
fn lower_mad(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 4)?;
    match raw.modifiers.as_slice() {
        [m1, m2] if m1 == "lo" && m2 == "s32" => {
            let a = reg_index(&raw.operands[0], raw.line)?;
            let b = reg_index(&raw.operands[1], raw.line)?;
            let c = reg_index(&raw.operands[2], raw.line)?;
            let d = reg_index(&raw.operands[3], raw.line)?;
            Ok(make_inst(InstType::MadLoS32, vec![a, b, c, d]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `fma.rn.f32 %dest, %a, %b, %c` or `fma.rm.f32 ...`.
fn lower_fma(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 4)?;
    let opcode = match raw.modifiers.as_slice() {
        [r, ty] if r == "rn" && ty == "f32" => InstType::FmaRnF32,
        [r, ty] if r == "rm" && ty == "f32" => InstType::FmaRmF32,
        _ => {
            return Err(ParseError::UnknownOpcode {
                line: raw.line,
                mnemonic: raw.mnemonic.clone(),
                modifiers: raw.modifiers.clone(),
            })
        }
    };
    let args: Vec<usize> = raw
        .operands
        .iter()
        .map(|op| reg_index(op, raw.line))
        .collect::<Result<_, _>>()?;
    Ok(make_inst(opcode, args))
}

/// `add.s32 %d, %a, %b` or `add.s32 %d, %a, <imm>`;
/// `add.s64 %d, %a, %b`;
/// `add.f32 %d, %a, %b` or `add.f32 %d, %a, 0f...`.
fn lower_add(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "s32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Register { .. } => {
                    let b = reg_index(&raw.operands[2], raw.line)?;
                    Ok(make_inst(InstType::AddS32, vec![d, a, b]))
                }
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::AddS32Imm,
                    vec![d, a, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "third operand of add.s32 must be register or immediate".to_string(),
                }),
            }
        }
        [ty] if ty == "s64" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::AddS64, args))
        }
        [ty] if ty == "f32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Register { .. } => {
                    let b = reg_index(&raw.operands[2], raw.line)?;
                    Ok(make_inst(InstType::AddF32, vec![d, a, b]))
                }
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::AddF32Imm,
                    vec![d, a, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "third operand of add.f32 must be register or immediate".to_string(),
                }),
            }
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_sub(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "f32" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::SubF32, args))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `mul.f32 %d, %a, %b` or `mul.wide.s32 %rd, %r, <imm>`.
///
/// Per the design note, `mul.wide.s32` always has an immediate third operand
/// in the PTX we've seen, so we emit `MulWideS32` assuming that. If a
/// register turns up in slot 2, we error.
fn lower_mul(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "f32" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::MulF32, args))
        }
        [m1, m2] if m1 == "wide" && m2 == "s32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Immediate(imm) => {
                    Ok(make_inst(InstType::MulWideS32, vec![d, a, imm_to_usize(*imm)]))
                }
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "mul.wide.s32 only supported with immediate third operand".to_string(),
                }),
            }
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_neg(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "f32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::NegF32, vec![d, a]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `setp.ge.s32 %p, %a, %b/<imm>` or `setp.lt.s32 %p, %a, %b/<imm>`.
fn lower_setp(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    let p = reg_index(&raw.operands[0], raw.line)?;
    let a = reg_index(&raw.operands[1], raw.line)?;
    match raw.modifiers.as_slice() {
        [cmp, ty] if cmp == "ge" && ty == "s32" => match &raw.operands[2] {
            RawOperand::Register { .. } => {
                let b = reg_index(&raw.operands[2], raw.line)?;
                Ok(make_inst(InstType::SetpGeS32, vec![p, a, b]))
            }
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpGeS32Imm,
                vec![p, a, imm_to_usize(*imm)],
            )),
            _ => Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "third operand of setp.ge.s32 must be register or immediate".to_string(),
            }),
        },
        [cmp, ty] if cmp == "lt" && ty == "s32" => match &raw.operands[2] {
            RawOperand::Register { .. } => {
                let b = reg_index(&raw.operands[2], raw.line)?;
                Ok(make_inst(InstType::SetpLtS32, vec![p, a, b]))
            }
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpLtS32Imm,
                vec![p, a, imm_to_usize(*imm)],
            )),
            _ => Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "third operand of setp.lt.s32 must be register or immediate".to_string(),
            }),
        },
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_or(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [m] if m == "pred" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::OrPred, args))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_shl(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "b32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            let b = match &raw.operands[2] {
                RawOperand::Immediate(imm) => imm_to_usize(*imm),
                RawOperand::Register { .. } => reg_index(&raw.operands[2], raw.line)?,
                _ => {
                    return Err(ParseError::UnsupportedOperandShape {
                        line: raw.line,
                        opcode: format_opcode(raw),
                        reason: "shl.b32 third operand must be register or immediate".to_string(),
                    })
                }
            };
            Ok(make_inst(InstType::ShlB32, vec![d, a, b]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `cvta.to.global.u64 %rd_dest, %rd_src` -> CvtaToGlobal [dest, src].
fn lower_cvta(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m1, m2, m3] if m1 == "to" && m2 == "global" && m3 == "u64" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let s = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::CvtaToGlobal, vec![d, s]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `cvt.sat.f32.f32 %f_dest, %f_src` -> CvtSatF32F32 [dest, src].
fn lower_cvt(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m1, m2, m3] if m1 == "sat" && m2 == "f32" && m3 == "f32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let s = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::CvtSatF32F32, vec![d, s]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_ex2(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m1, m2] if m1 == "approx" && m2 == "f32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let s = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::Ex2ApproxF32, vec![d, s]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

fn lower_rcp(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m1, m2] if m1 == "rn" && m2 == "f32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let s = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::RcpRnF32, vec![d, s]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `bra $L`, `@%p bra $L`, `@!%p bra $L`.
fn lower_branch(
    raw: &RawInstruction,
    guard: Option<PredGuard>,
    labels: &LabelMap,
) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 1)?;
    let target = match &raw.operands[0] {
        RawOperand::Label(name) => labels.get(name).copied().ok_or_else(|| {
            ParseError::UndefinedLabel {
                line: raw.line,
                label: name.clone(),
            }
        })?,
        _ => {
            return Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "bra target must be a label".to_string(),
            })
        }
    };

    match guard {
        Some(g) if !g.negated => Ok(make_inst(InstType::BraIf, vec![g.reg as usize, target])),
        Some(g) if g.negated => Ok(make_inst(
            InstType::BraIfNot,
            vec![g.reg as usize, target],
        )),
        None => Ok(make_inst(InstType::Bra, vec![target])),
        _ => unreachable!(),
    }
}

// ----------------------------------------------------------------------------
// Operand-encoding helpers
// ----------------------------------------------------------------------------

/// Encode a `RawOperand::Register` as its index. Errors on any other kind.
fn reg_index(op: &RawOperand, line: usize) -> Result<usize, ParseError> {
    match op {
        RawOperand::Register { index, .. } => Ok(*index as usize),
        _ => Err(ParseError::UnsupportedOperandShape {
            line,
            opcode: "<operand>".to_string(),
            reason: format!("expected register, found {op:?}"),
        }),
    }
}

/// Encode a `RawOperand::MemoryRef(Register)` as the address register index.
fn memref_reg(op: &RawOperand, line: usize) -> Result<usize, ParseError> {
    match op {
        RawOperand::MemoryRef(inner) => match &**inner {
            RawOperand::Register { index, .. } => Ok(*index as usize),
            other => Err(ParseError::UnsupportedOperandShape {
                line,
                opcode: "<memref>".to_string(),
                reason: format!("memory reference must wrap a register, found {other:?}"),
            }),
        },
        _ => Err(ParseError::UnsupportedOperandShape {
            line,
            opcode: "<memref>".to_string(),
            reason: format!("expected `[%reg]`, found {op:?}"),
        }),
    }
}

/// Resolve `[param_name]` to its index in the ParamMap.
fn param_ref(op: &RawOperand, params: &ParamMap, line: usize) -> Result<usize, ParseError> {
    let name = match op {
        RawOperand::MemoryRef(inner) => match &**inner {
            RawOperand::Identifier(s) => s,
            other => {
                return Err(ParseError::UnsupportedOperandShape {
                    line,
                    opcode: "ld.param".to_string(),
                    reason: format!("expected `[param_name]`, found `[{other:?}]`"),
                })
            }
        },
        _ => {
            return Err(ParseError::UnsupportedOperandShape {
                line,
                opcode: "ld.param".to_string(),
                reason: format!("expected `[param_name]`, found {op:?}"),
            })
        }
    };
    params.get(name).copied().ok_or_else(|| ParseError::UndefinedParam {
        line,
        param: name.clone(),
    })
}

/// Encode an immediate value as a usize. Integers are truncated/reinterpreted;
/// floats are stored as their IEEE 754 bit pattern.
fn imm_to_usize(imm: ImmediateValue) -> usize {
    match imm {
        ImmediateValue::IntZero => 0,
        // Truncate/sign-reinterpret i64 into usize. Zekai's executor is
        // expected to know the intended type based on the opcode and can
        // cast back accordingly.
        ImmediateValue::Int(v) => v as usize,
        ImmediateValue::F32Bits(bits) => bits as usize,
    }
}

/// Convenience: grab three operands as register indices.
fn collect_three_regs(raw: &RawInstruction) -> Result<Vec<usize>, ParseError> {
    Ok(vec![
        reg_index(&raw.operands[0], raw.line)?,
        reg_index(&raw.operands[1], raw.line)?,
        reg_index(&raw.operands[2], raw.line)?,
    ])
}

/// Enforce a specific operand count or error.
fn expect_operand_count(raw: &RawInstruction, n: usize) -> Result<(), ParseError> {
    if raw.operands.len() != n {
        Err(ParseError::UnsupportedOperandShape {
            line: raw.line,
            opcode: format_opcode(raw),
            reason: format!("expected {} operands, found {}", n, raw.operands.len()),
        })
    } else {
        Ok(())
    }
}

/// Format a mnemonic + modifiers back into a printable opcode string for
/// use in error messages.
fn format_opcode(raw: &RawInstruction) -> String {
    let mods: String = raw.modifiers.iter().map(|m| format!(".{m}")).collect();
    format!("{}{}", raw.mnemonic, mods)
}

/* 
// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::tokenize;
    use crate::parser::parser::parse_tokens;

    fn parse_and_lower(src: &str) -> ParsedKernel {
        let parse_out = parse_tokens(tokenize(src)).expect("parse should succeed");
        lower(parse_out).expect("lower should succeed")
    }

    #[test]
    fn ret_only() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions.len(), 1);
        assert_eq!(k.instructions[0].inst_type, InstType::Ret);
        assert!(k.instructions[0].args.is_empty());
    }

    #[test]
    fn mad_lo_s32() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                mad.lo.s32 %r1, %r5, %r4, %r3;
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::MadLoS32);
        assert_eq!(k.instructions[0].args, vec![1, 5, 4, 3]);
    }

    #[test]
    fn mov_tid_x() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                mov.u32 %r3, %tid.x;
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::MovTidX);
        assert_eq!(k.instructions[0].args, vec![3]);
    }

    #[test]
    fn mul_wide_with_immediate() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                mul.wide.s32 %rd5, %r1, 4;
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::MulWideS32);
        assert_eq!(k.instructions[0].args, vec![5, 1, 4]);
    }

    #[test]
    fn store_address_is_args_zero() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                st.global.f32 [%rd10], %f3;
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::StGlobalF32);
        // args[0] = address register, args[1] = value register
        assert_eq!(k.instructions[0].args, vec![10, 3]);
    }

    #[test]
    fn ld_param_resolves_arg_index() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo(
                .param .u64 foo_param_0,
                .param .u64 foo_param_1
            )
            {
                ld.param.u64 %rd2, [foo_param_1];
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::LdParamU64);
        // args[0] = dest register, args[1] = param index (1 for foo_param_1)
        assert_eq!(k.instructions[0].args, vec![2, 1]);
    }

    #[test]
    fn branch_resolves_label_to_pc() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                @%p1 bra $L_end;
                add.s64 %rd1, %rd2, %rd3;
            $L_end:
                ret;
            }
        "#,
        );
        // Real instructions: [0] BraIf, [1] AddS64, [2] Ret
        // Label $L_end points at instruction index 2.
        assert_eq!(k.instructions[0].inst_type, InstType::BraIf);
        assert_eq!(k.instructions[0].args, vec![1, 2]);
        assert_eq!(k.instructions[1].inst_type, InstType::AddS64);
        assert_eq!(k.instructions[2].inst_type, InstType::Ret);
    }

    #[test]
    fn unconditional_branch() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                bra $L_end;
            $L_end:
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::Bra);
        assert_eq!(k.instructions[0].args, vec![1]);
    }

    #[test]
    fn negated_branch() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                @!%p2 bra $L_end;
            $L_end:
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::BraIfNot);
        assert_eq!(k.instructions[0].args, vec![2, 1]);
    }

    #[test]
    fn add_s32_with_register_vs_immediate() {
        let k = parse_and_lower(
            r#"
            .visible .entry foo()
            {
                add.s32 %r1, %r2, %r3;
                add.s32 %r4, %r5, 10;
                ret;
            }
        "#,
        );
        assert_eq!(k.instructions[0].inst_type, InstType::AddS32);
        assert_eq!(k.instructions[0].args, vec![1, 2, 3]);
        assert_eq!(k.instructions[1].inst_type, InstType::AddS32Imm);
        assert_eq!(k.instructions[1].args, vec![4, 5, 10]);
    }

    #[test]
    fn predicate_guard_on_non_branch_errors() {
        let src = r#"
            .visible .entry foo()
            {
                @%p1 add.s32 %r1, %r2, %r3;
                ret;
            }
        "#;
        let parse_out = parse_tokens(tokenize(src)).expect("parse ok");
        let err = lower(parse_out).expect_err("should reject");
        match err {
            ParseError::UnsupportedOperandShape { .. } => {}
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn full_add_kernel_lowers() {
        // End-to-end: the full addKernel PTX from the project notes should
        // lower without error and produce the expected number of inst_info.
        let src = r#"
.version 9.1
.target sm_75
.address_size 64

.visible .entry _Z9addKernelPfS_S_i(
    .param .u64 _Z9addKernelPfS_S_i_param_0,
    .param .u64 _Z9addKernelPfS_S_i_param_1,
    .param .u64 _Z9addKernelPfS_S_i_param_2,
    .param .u32 _Z9addKernelPfS_S_i_param_3
)
{
    .reg .pred     %p<2>;
    .reg .f32     %f<4>;
    .reg .b32     %r<6>;
    .reg .b64     %rd<11>;

    ld.param.u64     %rd1, [_Z9addKernelPfS_S_i_param_0];
    ld.param.u64     %rd2, [_Z9addKernelPfS_S_i_param_1];
    ld.param.u64     %rd3, [_Z9addKernelPfS_S_i_param_2];
    ld.param.u32     %r2, [_Z9addKernelPfS_S_i_param_3];
    mov.u32     %r3, %tid.x;
    mov.u32     %r4, %ntid.x;
    mov.u32     %r5, %ctaid.x;
    mad.lo.s32     %r1, %r5, %r4, %r3;
    setp.ge.s32     %p1, %r1, %r2;
    @%p1 bra     $L__BB0_2;

    cvta.to.global.u64     %rd4, %rd1;
    mul.wide.s32     %rd5, %r1, 4;
    add.s64     %rd6, %rd4, %rd5;
    cvta.to.global.u64     %rd7, %rd2;
    add.s64     %rd8, %rd7, %rd5;
    ld.global.f32     %f1, [%rd8];
    ld.global.f32     %f2, [%rd6];
    add.f32     %f3, %f2, %f1;
    cvta.to.global.u64     %rd9, %rd3;
    add.s64     %rd10, %rd9, %rd5;
    st.global.f32     [%rd10], %f3;

$L__BB0_2:
    ret;
}
"#;
        let k = parse_and_lower(src);
        // 22 real instructions (the label pseudo doesn't become an inst_info).
        assert_eq!(k.instructions.len(), 22);
        // The label `$L__BB0_2` should resolve to PC = 21 (the Ret).
        // Branch at index 9 (after 4 ld.param, 3 mov, 1 mad, 1 setp = 9).
        assert_eq!(k.instructions[9].inst_type, InstType::BraIf);
        assert_eq!(k.instructions[9].args[1], 21); // target PC
        assert_eq!(k.instructions[21].inst_type, InstType::Ret);
    }
}

    */