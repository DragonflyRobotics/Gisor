//! Lowering: convert `ParseOutput` into the final `ParsedKernel`.
//!
//! This is where semantic decisions happen:
//!   - pick the right `InstType` variant based on (mnemonic, modifiers,
//!     operand shape)
//!   - rewrite special cases like `mov.u32 %r, %tid.x` -> `MovTidX`
//!   - resolve `$label` operands to PC values
//!   - resolve `[param_name]` operands to parameter placeholder indices
//!   - encode each operand into a `usize` slot for Zekai's `inst_info.args`
//!
//! The parsing phase did none of this — it only produced `RawInstruction`s
//! that faithfully reflect the PTX source. Lowering is where we commit to
//! Zekai's execution model.

use std::collections::HashMap;

use crate::parser::ir::{
    ImmediateValue, LabelMap, ParamMap, PredGuard, RawInstruction, RawOperand, RegBank, SpecialReg,
};
use crate::parser::error::ParseError;
use crate::parser::output::ParsedKernel;
use crate::parser::parser::ParseOutput;

// Import Zekai's instruction types directly from the `gpu` crate.
// `make_inst` is needed because `inst_info`'s fields are `pub(crate)`
// — we can't construct the struct from outside that crate.
use gpu::inst_info::{inst_info, make_inst};
use gpu::inst_type::InstType;

/// Top-level lowering entry point.
pub fn lower(parse_out: ParseOutput) -> Result<ParsedKernel, ParseError> {
    // Build the param name -> index map once, for use in every ld.param
    // lowering decision.
    let mut param_map: ParamMap = HashMap::new();
    for (i, p) in parse_out.params.iter().enumerate() {
        param_map.insert(p.name.clone(), i);
    }

    // Pass 1: build the label map by walking the raw instructions and
    // counting only real ones.
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

// ----------------------------------------------------------------------------
// Label map construction
// ----------------------------------------------------------------------------

/// Walk the raw instruction list once and build `label_name -> pc`. The PC
/// is the index of the next real (non-label) instruction. Multiple labels
/// can point at the same instruction (e.g. two labels with no instructions
/// between them).
fn build_label_map(raw: &[RawInstruction]) -> LabelMap {
    let mut map = LabelMap::new();
    let mut pc: usize = 0;
    for inst in raw {
        if inst.mnemonic == ".label" {
            // The label points at the NEXT real instruction's PC, which is
            // the current `pc` since `.label` entries don't increment it.
            if let Some(RawOperand::Label(s)) = inst.operands.first() {
                map.insert(s.clone(), pc);
            }
        } else {
            pc += 1;
        }
    }
    map
}

// ----------------------------------------------------------------------------
// Per-instruction lowering
// ----------------------------------------------------------------------------

fn lower_instruction(
    raw: &RawInstruction,
    labels: &LabelMap,
    params: &ParamMap,
) -> Result<inst_info, ParseError> {
    // Branches are the only instructions that legitimately carry a predicate
    // guard in our supported subset. Reject guards on anything else.
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
        "div" => lower_div(raw),
        "mul" => lower_mul(raw),
        "neg" => lower_neg(raw),
        "setp" => lower_setp(raw),
        "or" => lower_or(raw),
        "and" => lower_and(raw),
        "xor" => lower_xor(raw),
        "not" => lower_not(raw),
        "shl" => lower_shl(raw),
        "shr" => lower_shr(raw),
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

// ----------------------------------------------------------------------------
// Opcode-family lowering helpers
// ----------------------------------------------------------------------------

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
        // ld.global.u32 %r, [%rd]
        [m, ty] if m == "global" && ty == "u32" => {
            let addr = memref_reg(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::LdGlobalU32, vec![dest, addr]))
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
        [m, ty] if m == "global" && ty == "u32" => {
            let addr = memref_reg(&raw.operands[0], raw.line)?;
            let value = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::StGlobalU32, vec![addr, value]))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `mov` has several forms:
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
        [ty] if ty == "pred" => {
            // mov.pred %p, <imm>  -- set predicate to 0 or 1.
            let dest = reg_index(dest_op, raw.line)?;
            match src_op {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::MovPred,
                    vec![dest, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "mov.pred source must be an immediate".to_string(),
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
/// Also handles `mad.lo.s32 %dest, %a, imm, imm` -> MadLoS32Imm
/// (common pattern for affine address arithmetic: dst = a * imm1 + imm2).
fn lower_mad(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 4)?;
    match raw.modifiers.as_slice() {
        [m1, m2] if m1 == "lo" && m2 == "s32" => {
            // Check operand shape to pick the right variant.
            let all_regs = raw.operands.iter().all(|op| matches!(op, RawOperand::Register { .. }));
            if all_regs {
                let a = reg_index(&raw.operands[0], raw.line)?;
                let b = reg_index(&raw.operands[1], raw.line)?;
                let c = reg_index(&raw.operands[2], raw.line)?;
                let d = reg_index(&raw.operands[3], raw.line)?;
                return Ok(make_inst(InstType::MadLoS32, vec![a, b, c, d]));
            }
            // Try the reg+reg+imm+imm pattern: dst reg, src reg, then two immediates.
            let dst_is_reg = matches!(&raw.operands[0], RawOperand::Register { .. });
            let a_is_reg = matches!(&raw.operands[1], RawOperand::Register { .. });
            let b_is_imm = matches!(&raw.operands[2], RawOperand::Immediate(_));
            let c_is_imm = matches!(&raw.operands[3], RawOperand::Immediate(_));
            if dst_is_reg && a_is_reg && b_is_imm && c_is_imm {
                let dst = reg_index(&raw.operands[0], raw.line)?;
                let a = reg_index(&raw.operands[1], raw.line)?;
                let b = match &raw.operands[2] {
                    RawOperand::Immediate(imm) => imm_to_usize(*imm),
                    _ => unreachable!(),
                };
                let c = match &raw.operands[3] {
                    RawOperand::Immediate(imm) => imm_to_usize(*imm),
                    _ => unreachable!(),
                };
                return Ok(make_inst(InstType::MadLoS32Imm, vec![dst, a, b, c]));
            }
            Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "mad.lo.s32 supports either 4 registers or (dst_r, src_r, imm, imm)"
                    .to_string(),
            })
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

fn lower_div(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [r, ty] if r == "rn" && ty == "f32" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::DivRnF32, args))
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
        [cmp, ty] if cmp == "le" && ty == "f32" => match &raw.operands[2] {
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpLeF32Imm,
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
        [cmp, ty] if cmp == "eq" && ty == "b32" => match &raw.operands[2] {
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpEqB32,
                vec![p, a, imm_to_usize(*imm)],
            )),
            _ => Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "setp.eq.b32 third operand must be immediate".to_string(),
            }),
        },
        [cmp, ty] if cmp == "eq" && ty == "s32" => match &raw.operands[2] {
            RawOperand::Register { .. } => {
                let b = reg_index(&raw.operands[2], raw.line)?;
                Ok(make_inst(InstType::SetpEqS32, vec![p, a, b]))
            }
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpEqS32Imm,
                vec![p, a, imm_to_usize(*imm)],
            )),
            _ => Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "third operand of setp.eq.s32 must be register or immediate".to_string(),
            }),
        },
        [cmp, ty] if cmp == "ne" && ty == "s32" => match &raw.operands[2] {
            RawOperand::Register { .. } => {
                let b = reg_index(&raw.operands[2], raw.line)?;
                Ok(make_inst(InstType::SetpNeS32, vec![p, a, b]))
            }
            RawOperand::Immediate(imm) => Ok(make_inst(
                InstType::SetpNeS32Imm,
                vec![p, a, imm_to_usize(*imm)],
            )),
            _ => Err(ParseError::UnsupportedOperandShape {
                line: raw.line,
                opcode: format_opcode(raw),
                reason: "third operand of setp.ne.s32 must be register or immediate".to_string(),
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

/// `shr.u32 %r, %r, <imm>` or `shr.s32 %r, %r, <imm>`.
/// Signed vs unsigned matters because of sign extension.
fn lower_shr(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "u32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::ShrU32,
                    vec![d, a, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "shr.u32 third operand must be immediate".to_string(),
                }),
            }
        }
        [ty] if ty == "s32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::ShrS32,
                    vec![d, a, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "shr.s32 third operand must be immediate".to_string(),
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

/// `and.b32 %r, %r, %r` or `and.b32 %r, %r, <imm>`.
///
/// We emit two distinct opcodes so the executor knows unambiguously
/// whether args[2] is a register index or a literal value. Without this
/// distinction, the value `1` could mean either "register %r1" or "the
/// immediate 1", which would silently miscompute.
fn lower_and(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [ty] if ty == "b32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            match &raw.operands[2] {
                RawOperand::Register { .. } => {
                    let b = reg_index(&raw.operands[2], raw.line)?;
                    Ok(make_inst(InstType::AndB32, vec![d, a, b]))
                }
                RawOperand::Immediate(imm) => Ok(make_inst(
                    InstType::AndB32Imm,
                    vec![d, a, imm_to_usize(*imm)],
                )),
                _ => Err(ParseError::UnsupportedOperandShape {
                    line: raw.line,
                    opcode: format_opcode(raw),
                    reason: "and.b32 third operand must be register or immediate".to_string(),
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

/// `xor.pred %p, %p, %p` -- logical XOR on predicates.
fn lower_xor(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 3)?;
    match raw.modifiers.as_slice() {
        [m] if m == "pred" => {
            let args = collect_three_regs(raw)?;
            Ok(make_inst(InstType::XorPred, args))
        }
        _ => Err(ParseError::UnknownOpcode {
            line: raw.line,
            mnemonic: raw.mnemonic.clone(),
            modifiers: raw.modifiers.clone(),
        }),
    }
}

/// `not.pred %p, %p` -- logical NOT on a predicate.
fn lower_not(raw: &RawInstruction) -> Result<inst_info, ParseError> {
    expect_operand_count(raw, 2)?;
    match raw.modifiers.as_slice() {
        [m] if m == "pred" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let a = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::NotPred, vec![d, a]))
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
        [m1, m2, m3] if m1 == "rn" && m2 == "f32" && m3 == "s32" => {
            let d = reg_index(&raw.operands[0], raw.line)?;
            let s = reg_index(&raw.operands[1], raw.line)?;
            Ok(make_inst(InstType::CvtRnF32S32, vec![d, s]))
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
        None => {
            // Distinguish `bra` (conditional-ready) from `bra.uni` (warp-uniform
            // unconditional). We treat them as separate opcodes per Zekai's enum.
            let is_uni = raw.modifiers.iter().any(|m| m == "uni");
            if is_uni {
                Ok(make_inst(InstType::BraUni, vec![target]))
            } else {
                Ok(make_inst(InstType::Bra, vec![target]))
            }
        }
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
// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn float_immediate_bit_pattern_preserved() {
        // The bit pattern for 1.0f32 is 0x3F800000 = 1065353216.
        // If `imm_to_usize` handles F32Bits correctly, then after lowering,
        // args[1] of a mov.f32 should equal 0x3F800000.
        // Then if the executor does `f32::from_bits(args[1] as u32)`, it
        // should recover exactly 1.0.
        let src = r#"
            .visible .entry k()
            {
                mov.f32 %f1, 0f3F800000;
                ret;
            }
        "#;
        let out = crate::parser::parse(src).unwrap();
        let mov = &out.instructions[0];

        // Check the raw encoded value.
        assert_eq!(
            mov.args[1], 0x3F800000,
            "float bits should round-trip as 0x3F800000"
        );

        // Check that reinterpretation produces 1.0.
        let bits = mov.args[1] as u32;
        let value = f32::from_bits(bits);
        assert_eq!(value, 1.0_f32, "recovered value should be 1.0");
    }

    #[test]
    fn float_immediate_negative_value() {
        // -1.0f32 is 0xBF800000 = 3212836864. High bit is set, which is the
        // scenario most likely to expose sign-extension bugs in usize conversion.
        let src = r#"
            .visible .entry k()
            {
                mov.f32 %f1, 0fBF800000;
                ret;
            }
        "#;
        let out = crate::parser::parse(src).unwrap();
        let mov = &out.instructions[0];

        assert_eq!(
            mov.args[1] as u32,
            0xBF800000,
            "negative float bits should round-trip"
        );

        let value = f32::from_bits(mov.args[1] as u32);
        assert_eq!(value, -1.0_f32);
    }

    #[test]
    fn float_immediate_in_add_f32_imm() {
        // Also test immediates in binary ops, not just mov.
        // 2.5f32 = 0x40200000 = 1075838976.
        let src = r#"
            .visible .entry k()
            {
                add.f32 %f1, %f2, 0f40200000;
                ret;
            }
        "#;
        let out = crate::parser::parse(src).unwrap();
        let add = &out.instructions[0];
        // args = [dst, src_reg, imm_bits]
        assert_eq!(add.args[2] as u32, 0x40200000);
        assert_eq!(f32::from_bits(add.args[2] as u32), 2.5_f32);
    }
}