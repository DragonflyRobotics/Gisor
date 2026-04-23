use std::collections::HashMap;

use crate::parser::ir::{
    inst_info, ImmediateValue, InstType, LabelMap, ParamMap, PredGuard, RawInstruction, RawOperand,
    RegBank, SpecialReg,
};
use crate::parser::error::ParseError;
use crate::parser::output::ParsedKernel;
use crate::parser::parser::ParseOutput;
use gpu::inst_info::make_inst;

///converts intermediate raw param into final parsed param
pub fn lower(parse_out: ParseOutput) -> Result<ParsedKernel, ParseError> {
    //build the param name -> index map
    let mut param_map: ParamMap = HashMap::new();
    for (i, p) in parse_out.params.iter().enumerate() {
        param_map.insert(p.name.clone(), i);
    }

    //pass 1 - build the label map by walking the raw instructions
    let label_map = build_label_map(&parse_out.raw_instructions);

    //pass 2 - emit inst_info for each real instruction
    let mut instructions = Vec::new();
    for raw in &parse_out.raw_instructions {
        if raw.mnemonic == ".label" {
            continue; //pseudo-instructions not emitted
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


//OPCODE LOWERING HELPERS
fn lower_ld(raw: &RawInstruction, params: &ParamMap) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    let dest = reg_index(&raw.operands[0], raw.line)?;

    match raw.modifiers.as_slice() {
        //ld.param.u64
        [m, ty] if m == "param" && ty == "u64" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamU64, vec![dest, arg_idx]))
        }
        //ld.param.u32
        [m, ty] if m == "param" && ty == "u32" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamU32, vec![dest, arg_idx]))
        }
        //ld.param.f32
        [m, ty] if m == "param" && ty == "f32" => {
            let arg_idx = param_ref(&raw.operands[1], params, raw.line)?;
            Ok(make_inst(InstType::LdParamF32, vec![dest, arg_idx]))
        }
        //ld.global.f32
        [m, ty] if m == "global" && ty == "f32" => {
            let addr = memref_reg(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::LdGlobalF32, vec![dest, addr]))
        }
        // ld.global.u32 %r, [%rd]
        [m, ty] if m == "global" && ty == "u32" => {
            let addr = memref_reg(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::LdGlobalU32, vec![dest, addr]))
        }
        //ld.global.nc.f32
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

fn lower_mov(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    let dest_op = &raw.operands[0];
    let src_op = &raw.operands[1];

    //special register sources
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

    //seperate by type modifier
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
            //mov.b32 %r, %f
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



///OPERAND ENCODING HELPERS
/// Encode a `RawOperand::Register` as its index
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
