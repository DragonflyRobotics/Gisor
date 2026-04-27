#include "lowering.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static bool find_label(const RawInstruction* raws, size_t n, const char* name, size_t* out_pc) {
    size_t pc = 0;
    for (size_t i = 0; i < n; i++) {
        if (!strcmp(raws[i].instr_name, ".label")) {
            if (raws[i].operand_count > 0
             && raws[i].operands[0].type == RAW_OP_LABEL
             && !strcmp(raws[i].operands[0].data.label, name)) {
                *out_pc = pc;
                return true;
            }
        } else {
            pc++;
        }
    }
    return false;
}

static size_t reg_index(const RawInstruction* r, const RawOperand* op) {
    return (size_t)op->data.reg.index;
}

static size_t imm_to_usize(const ImmValue* v) {
    switch (v->type) {
    case IMM_INT_ZERO: return 0;
    case IMM_INT: return (size_t)(uint64_t)v->data.int_val;
    case IMM_F32_BITS: return (size_t)v->data.f32_bits;
    }
    return 0;
}



//INSTRUCTION LOWERING HELPERS
static InstInfo lower_ret(const RawInstruction* r) {
    (void)r;
    InstInfo i = {0};
    i.opcode = Ret;
    i.arg_count = 0;
    return i;
}


static InstInfo lower_st(const RawInstruction* r) {
    InstInfo i = {0};
    const RawOperand* addr = r->operands[0].data.mem_ref;
    const RawOperand* src  = &r->operands[1];

    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "global")) {
        const char* ty = r->modifiers[1];
        if (!strcmp(ty, "u32")) i.opcode = StGlobalU32;
        else if (!strcmp(ty, "f32")) i.opcode = StGlobalF32;

        i.args[0] = (size_t)addr->data.reg.index;
        i.args[1] = (size_t)src->data.reg.index;
        i.arg_count = 2;
        return i;
    }

    return i;
}


static size_t encode_src_operand(const RawInstruction* r, const RawOperand* src) {
    if (src->type == RAW_OP_REGISTER) return (size_t)src->data.reg.index;
    if (src->type == RAW_OP_IMMEDIATE) return imm_to_usize(&src->data.imm);
    return 0;
}

static InstType special_reg_to_mov_opcode(SpecialReg sr) {
    switch (sr) {
    case SREG_TID_X: return MovTidX;
    case SREG_TID_Y: return MovTidY;
    case SREG_TID_Z: return MovTidZ;
    case SREG_NTID_X: return MovNtidX;
    case SREG_NTID_Y: return MovNtidY;
    case SREG_NTID_Z: return MovNtidZ;
    case SREG_CTAID_X: return MovCtaidX;
    case SREG_CTAID_Y: return MovCtaidY;
    case SREG_CTAID_Z: return MovCtaidZ;
    case SREG_NCTAID_X: return MovNctaidX;
    case SREG_NCTAID_Y: return MovNctaidY;
    case SREG_NCTAID_Z: return MovNctaidZ;
    }
    return NoOp;
}

static InstType mov_opcode(const RawInstruction* r, const char* ty, const RawOperand* src) {
    if (!strcmp(ty, "u32")) {
        if (src->type == RAW_OP_IMMEDIATE) return MovU32Imm;
        if (src->type == RAW_OP_REGISTER) return MovU32;
    }
    if (!strcmp(ty, "u64")) {
        if (src->type == RAW_OP_IMMEDIATE) return MovU64Imm;
        if (src->type == RAW_OP_REGISTER) return MovU64;
    }
    if (!strcmp(ty, "f32")) {
        if (src->type == RAW_OP_IMMEDIATE) {
            return src->data.imm.type == IMM_F32_BITS ? MovF32Bits : MovF32Imm;
        }
        if (src->type == RAW_OP_REGISTER) {
            return src->data.reg.bank == REG_BANK_F ? MovF32 : MovF32FromB32;
        }
    }
    if (!strcmp(ty, "b32")) {
        if (src->type == RAW_OP_REGISTER && src->data.reg.bank == REG_BANK_F) {
            return MovB32FromF32;
        }
    }
    if (!strcmp(ty, "pred")) {
        if (src->type == RAW_OP_REGISTER && src->data.reg.bank == REG_BANK_P) {
            return MovPred;
        }
    }
    return NoOp; //unsupported
}

static InstInfo lower_mov(const RawInstruction* r) {
    InstInfo i = {0};
    size_t dst = reg_index(r, &r->operands[0]);
    const RawOperand* src = &r->operands[1];

    //special reg case
    if (src->type == RAW_OP_SPECIAL_REG) {
        i.opcode = special_reg_to_mov_opcode(src->data.special_reg);
        i.args[0] = dst;
        i.arg_count = 1;
        return i;
    }

    i.opcode = mov_opcode(r, r->modifiers[0], src);
    i.args[0] = dst;
    i.args[1] = encode_src_operand(r, src);
    i.arg_count = 2;
    return i;
}

static InstInfo lower_branch(const RawInstruction* r, const RawInstruction* raws, size_t raw_count) {
    InstInfo i = {0};
    size_t pc;
    i.opcode = Bra;
    i.args[0] = pc;
    i.arg_count = 1;
    return i;
}


//load logic
static bool find_param(const ParamInfo* params, size_t n, const char* name, size_t* out_idx) {
    for (size_t i = 0; i < n; i++) {
        if (!strcmp(params[i].name, name)) { 
            *out_idx = i; return true;
        }
    }
    return false;
}

static InstInfo lower_ld(const RawInstruction* r, const ParamInfo* params, size_t param_count) {
    InstInfo i = {0};
    size_t dst = reg_index(r, &r->operands[0]);
    const RawOperand* inner = r->operands[1].data.mem_ref;
    
    //load param
    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "param")) {
        size_t param_idx;
        if (!find_param(params, param_count, inner->data.id, &param_idx)) {
            fprintf(stderr, "ld.param: unknown parameter name '%s'\n", inner->data.id);
            abort();
        }
       
        const char* ty = r->modifiers[1];
       
        if (!strcmp(ty, "u64")) i.opcode = LdParamU64;
        else if (!strcmp(ty, "u32")) i.opcode = LdParamU32;
        else if (!strcmp(ty, "f32")) i.opcode = LdParamF32;

        i.args[0] = dst;
        i.args[1] = param_idx;
        i.arg_count = 2;
        return i;
    }

    //load global
    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "global")) {
        const char* ty = r->modifiers[1];
        if (!strcmp(ty, "u32")) i.opcode = LdGlobalU32;
        else if (!strcmp(ty, "f32")) i.opcode = LdGlobalF32;

        i.args[0] = dst;
        i.args[1] = (size_t)inner->data.reg.index;
        i.arg_count = 2;
        return i;
    }

    //noncoherent global load
    if (r->modifier_count == 3 && !strcmp(r->modifiers[0], "global") && !strcmp(r->modifiers[1], "nc")) {
        const char* ty = r->modifiers[2];
        i.opcode = LdGlobalNcF32;

        i.args[0] = dst;
        i.args[1] = (size_t)inner->data.reg.index;
        i.arg_count = 2;
        return i;
    }

    return i; 
}

static InstInfo lower_mad(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;
    const RawOperand* c = &r->operands[3];

    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "lo") && !strcmp(r->modifiers[1], "s32")) {
        if (c->type == RAW_OP_IMMEDIATE) {
            i.opcode = MadLoS32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = b;
            i.args[3] = imm_to_usize(&c->data.imm);
            i.arg_count = 4;
            return i;
        }
        i.opcode = MadLoS32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.args[3] = (size_t)c->data.reg.index;
        i.arg_count = 4;
        return i;
    }

    return i;
}


static InstInfo lower_fma(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;
    size_t c = (size_t)r->operands[3].data.reg.index;

    if (r->modifier_count == 2 && !strcmp(r->modifiers[1], "f32")) {
        if (!strcmp(r->modifiers[0], "rn")) i.opcode = FmaRnF32;
        else if (!strcmp(r->modifiers[0], "rm")) i.opcode = FmaRmF32;

        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.args[3] = c;
        i.arg_count = 4;
        return i;
    }

    return i;
}


static InstInfo lower_add(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    const RawOperand* b = &r->operands[2];

    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "s32")) {
        if (b->type == RAW_OP_IMMEDIATE) {
            i.opcode = AddS32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = imm_to_usize(&b->data.imm);
            i.arg_count = 3;
            return i;
        }
        i.opcode = AddS32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }
    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "s64")) {
        i.opcode = AddS64;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }
    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "f32")) {
        if (b->type == RAW_OP_IMMEDIATE) {
            i.opcode = AddF32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = imm_to_usize(&b->data.imm);
            i.arg_count = 3;
            return i;
        }
        i.opcode = AddF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }
    return i;
}

static InstInfo lower_sub(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    //only subf32 implemented for now
    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "f32")) {
        i.opcode = SubF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    return i;
}

static InstInfo lower_div(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    //only divrnf32 implemented for now
    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "rn") && !strcmp(r->modifiers[1], "f32")) {
        i.opcode = DivRnF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    return i;
}

static InstInfo lower_mul(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "f32")) {
        i.opcode = MulF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    if (r->modifier_count == 2 && !strcmp(r->modifiers[0], "wide")  && !strcmp(r->modifiers[1], "s32")) {
        i.opcode = MulWideS32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }

    return i;
}


static InstInfo lower_neg(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    //only negf32 supported
    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "f32")) {
        i.opcode = DivRnF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    return i;
}

static InstInfo lower_setp(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    const RawOperand* b = &r->operands[2];

    if (r->modifier_count != 2) return i;
    const char* cmp = r->modifiers[0];
    const char* ty = r->modifiers[1];

    if (!strcmp(cmp, "eq")) {
        if (!strcmp(ty, "s32")) {
            if (b->type == RAW_OP_IMMEDIATE) {
                i.opcode = SetpEqS32Imm;
                i.args[0] = dst;
                i.args[1] = a;
                i.args[2] = imm_to_usize(&b->data.imm);
                i.arg_count = 3;
                return i;
            }
            i.opcode = SetpEqS32;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = (size_t)b->data.reg.index;
            i.arg_count = 3;
            return i;
        }
        if (!strcmp(ty, "b32")) {
            i.opcode = SetpEqB32;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = (size_t)b->data.reg.index;
            i.arg_count = 3;
            return i;
        }
    }
    if (!strcmp(cmp, "ne") && !strcmp(ty, "s32")) {
        if (b->type == RAW_OP_IMMEDIATE) {
            i.opcode = SetpNeS32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = imm_to_usize(&b->data.imm);
            i.arg_count = 3;
            return i;
        }
        i.opcode = SetpNeS32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }
    if (!strcmp(cmp, "ge") && !strcmp(ty, "s32")) {
        if (b->type == RAW_OP_IMMEDIATE) {
            i.opcode = SetpGeS32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = imm_to_usize(&b->data.imm);
            i.arg_count = 3;
            return i;
        }
        i.opcode = SetpGeS32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }
    if (!strcmp(cmp, "lt")) {
        if (!strcmp(ty, "s32")) {
            if (b->type == RAW_OP_IMMEDIATE) {
                i.opcode = SetpLtS32Imm;
                i.args[0] = dst;
                i.args[1] = a;
                i.args[2] = imm_to_usize(&b->data.imm);
                i.arg_count = 3;
                return i;
            }
            i.opcode = SetpLtS32;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = (size_t)b->data.reg.index;
            i.arg_count = 3;
            return i;
        }
        if (!strcmp(ty, "u32")) {
            if (b->type == RAW_OP_IMMEDIATE) {
                i.opcode = SetpLtU32Imm;
                i.args[0] = dst;
                i.args[1] = a;
                i.args[2] = imm_to_usize(&b->data.imm);
                i.arg_count = 3;
                return i;
            }
            i.opcode = SetpLtU32;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = (size_t)b->data.reg.index;
            i.arg_count = 3;
            return i;
        }
    }
    if (!strcmp(cmp, "le") && !strcmp(ty, "f32")) {
        if (b->type == RAW_OP_IMMEDIATE) {
            i.opcode = SetpLeF32Imm;
            i.args[0] = dst;
            i.args[1] = a;
            i.args[2] = imm_to_usize(&b->data.imm);
            i.arg_count = 3;
            return i;
        }
        i.opcode = SetpLeF32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = (size_t)b->data.reg.index;
        i.arg_count = 3;
        return i;
    }

    return i;
}


static InstInfo lower_cvta(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t src = (size_t)r->operands[1].data.reg.index;

    if (r->modifier_count >= 2 && !strcmp(r->modifiers[0], "to") && !strcmp(r->modifiers[1], "global")) {
        i.opcode = CvtaToGlobal;
        i.args[0] = dst;
        i.args[1] = src;
        i.arg_count = 2;
        return i;
    }
    return i;
}

static InstInfo lower_or(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "pred")) {
        i.opcode = OrPred;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    return i;
}


static InstInfo lower_shl(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "b32")) {
        i.opcode = ShlB32;
        i.args[0] = dst;
        i.args[1] = a;
        i.args[2] = b;
        i.arg_count = 3;
        return i;
    }
    return i;
}

static InstInfo lower_shr(const RawInstruction* r) {
    InstInfo i = {0};

    size_t dst = (size_t)r->operands[0].data.reg.index;
    size_t a = (size_t)r->operands[1].data.reg.index;
    size_t b = (size_t)r->operands[2].data.reg.index;

    i.args[0] = dst;
    i.args[1] = a;
    i.args[2] = b;
    i.arg_count = 3;

    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "u32")) {
        i.opcode = ShrU32;
        return i;
    }
    if (r->modifier_count == 1 && !strcmp(r->modifiers[0], "s32")) {
        i.opcode = ShrS32;
        return i;
    }
    return i;
}








static InstInfo lower_instruction(const RawInstruction* r, const RawInstruction* raws, size_t raw_count, 
    const ParamInfo* params, size_t param_count) {

    (void)params;
    (void)param_count;

    if (!strcmp(r->instr_name, "ld")) return lower_ld(r, params, param_count);
    if (!strcmp(r->instr_name, "st")) return lower_st(r);
    if (!strcmp(r->instr_name, "fma")) return lower_fma(r);
    if (!strcmp(r->instr_name, "mad")) return lower_mad(r);
    if (!strcmp(r->instr_name, "add")) return lower_add(r);
    if (!strcmp(r->instr_name, "sub")) return lower_sub(r);
    if (!strcmp(r->instr_name, "mul")) return lower_mul(r);
    if (!strcmp(r->instr_name, "div")) return lower_div(r);
    if (!strcmp(r->instr_name, "neg")) return lower_neg(r);
    if (!strcmp(r->instr_name, "setp")) return lower_setp(r);
    if (!strcmp(r->instr_name, "or")) return lower_or(r);
    if (!strcmp(r->instr_name, "cvta")) return lower_cvta(r);
    if (!strcmp(r->instr_name, "ret")) return lower_ret(r);
    if (!strcmp(r->instr_name, "mov")) return lower_mov(r);
    if (!strcmp(r->instr_name, "bra")) return lower_branch(r, raws, raw_count);
    
    InstInfo dummy = {0};
    return dummy;
}

ParsedKernel lower(ParseOutput parsed, Arena* arena) {
    (void)arena;

    ParsedKernel out = {0};
    out.name = parsed.name;
    out.params = parsed.params;
    out.param_count = parsed.param_count;
    out.instructions = (InstInfo*)malloc(parsed.raw_count * sizeof(InstInfo));

    for (size_t i = 0; i < parsed.raw_count; i++) {
        const RawInstruction* r = &parsed.raw_instructions[i];
        if (!strcmp(r->instr_name, ".label")) continue;
        out.instructions[out.instruction_count++] = 
            lower_instruction(r, parsed.raw_instructions, parsed.raw_count, parsed.params, parsed.param_count);
    }
    return out;
}