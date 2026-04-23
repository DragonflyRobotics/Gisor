use memory::{Memory, MemoryAddress};
use crate::inst_type::InstType;
use crate::inst_info::inst_info;

#[derive(Clone)]
pub struct execute_unit {
    pub p: [bool; 256], // predicate registers
    pub r: [u32; 256], // gen purpose regs
    pub rd: [u64; 256], // double regs
    pub f: [f32; 256], // floating point regs

    tid_x: u32, // threadId.x
    tid_y: u32, // threadId.y
    tid_z: u32, // threadId.z

    ctaid_x: u32, // blockId.x
    ctaid_y: u32, // blockId.y
    ctaid_z: u32, // blockId.z

    ntid_x: u32, // blockDim.x
    ntid_y: u32, // blockDim.y
    ntid_z: u32, // blockDim.z

    nctaid_x: u32, // gridDim.x
    nctaid_y: u32, // gridDim.y
    nctaid_z: u32, // gridDim.z

    pc: u32, // cur inst running
    total_number_inst: u32,

    inst_list: Vec<inst_info>,
    branch_is_taken: bool,
}

impl Default for execute_unit {
    fn default() -> Self {
        Self {
            p:  [false; 256],
            r:  [0; 256],
            rd: [0; 256],
            f:  [0.0; 256],

            tid_x: 0, // threadId.x
            tid_y: 0, // threadId.y
            tid_z: 0, // threadId.z

            ctaid_x: 0, // blockId.x
            ctaid_y: 0, // blockId.y
            ctaid_z: 0, // blockId.z

            ntid_x: 0, // blockDim.x
            ntid_y: 0, // blockDim.y
            ntid_z: 0, // blockDim.z

            nctaid_x: 0, // gridDim.x
            nctaid_y: 0, // gridDim.y
            nctaid_z: 0, // gridDim.z

            pc: 0,
            total_number_inst: 0,

            inst_list: Vec::new(),
            branch_is_taken: false,
        }
    }
}
// pub fn run_demo(args1: Vec<usize>, args2: Vec<usize>, args3: Vec<usize>, args4: Vec<usize>, args5: Vec<usize>) {
//     let mut insts: Vec<inst_info> = Vec::new();
//     insts.push(inst_info { inst_type: InstType::LdParamU64, args: args1});
//     insts.push(inst_info { inst_type: InstType::CvtaToGlobal, args: args2});
//     insts.push(inst_info { inst_type: InstType::MulWideS32, args: args3});
//     insts.push(inst_info { inst_type: InstType::AddS64, args: args4});
//     insts.push(inst_info { inst_type: InstType::LdGlobalF32, args: args5});

//     let mut executor: execute_unit = execute_unit::new();
//     executor.inst_list = insts;
    // executor.execute_all();
// }

impl execute_unit {
    pub(crate) fn new() -> Self {
        Self {
            p:  [false; 256],
            r:  [0; 256],
            rd: [0; 256],
            f:  [0.0; 256],

            tid_x: 0, // threadId.x
            tid_y: 0, // threadId.y
            tid_z: 0, // threadId.z

            ctaid_x: 0, // blockId.x
            ctaid_y: 0, // blockId.y
            ctaid_z: 0, // blockId.z

            ntid_x: 0, // blockDim.x
            ntid_y: 0, // blockDim.y
            ntid_z: 0, // blockDim.z

            nctaid_x: 0, // nctaid.x
            nctaid_y: 0, // nctaid.y
            nctaid_z: 0, // nctaid.z

            pc: 0,
            total_number_inst: 0,

            inst_list: Vec::new(),
            branch_is_taken: false,
        }
    }

    pub fn set_execute_id(&mut self,
                      // thread position within block
                      tid_x: u32, tid_y: u32, tid_z: u32,
                      // block position within grid
                      ctaid_x: u32, ctaid_y: u32, ctaid_z: u32,
                      // block dimensions
                      ntid_x: u32, ntid_y: u32, ntid_z: u32,
                      // grid dimensions
                      nctaid_x: u32, nctaid_y: u32, nctaid_z: u32) {
        self.tid_x = tid_x;
        self.tid_y = tid_y;
        self.tid_z = tid_z;

        self.ctaid_x = ctaid_x;
        self.ctaid_y = ctaid_y;
        self.ctaid_z = ctaid_z;

        self.ntid_x = ntid_x;
        self.ntid_y = ntid_y;
        self.ntid_z = ntid_z;

        self.nctaid_x = nctaid_x;
        self.nctaid_y = nctaid_y;
        self.nctaid_z = nctaid_z;
    }

    pub fn import_inst(&mut self, instructions: Vec<inst_info>) {
        self.inst_list = instructions;
        self.total_number_inst = self.inst_list.len() as u32;
        self.pc = 0;
    }

    // For Debugging a single inst
    fn execute_single_inst(&mut self, inst: inst_info, mem: &mut Memory, args: Vec<usize>) {
        let a = &inst.args; // shorthand
        // println!("{:?}", inst.inst_type);
        // println!("DEBUG: args = {:?}", a);
        match inst.inst_type {
            // --- Loads ---
            InstType::LdParamU64 => self.load_param_u64(a[0], a[1] as u64, mem, args),
            InstType::LdParamU32 => self.load_param_u32(a[0], a[1] as u64, mem, args),
            InstType::LdParamF32 => self.load_param_f32(a[0], a[1] as u64, mem, args),

            // --- Mov special registers ---
            InstType::MovTidX => self.mov_u32_tid_x(a[0]),
            InstType::MovTidY => self.mov_u32_tid_y(a[0]),
            InstType::MovTidZ => self.mov_u32_tid_z(a[0]),
            InstType::MovCtaidX => self.mov_u32_ctaid_x(a[0]),
            InstType::MovCtaidY => self.mov_u32_ctaid_y(a[0]),
            InstType::MovCtaidZ => self.mov_u32_ctaid_z(a[0]),
            InstType::MovNtidX => self.mov_u32_ntid_x(a[0]),
            InstType::MovNtidY => self.mov_u32_ntid_y(a[0]),
            InstType::MovNtidZ => self.mov_u32_ntid_z(a[0]),
            InstType::MovNctaidX => self.mov_u32_nctaid_x(a[0]),
            InstType::MovNctaidY => self.mov_u32_nctaid_y(a[0]),
            InstType::MovNctaidZ => self.mov_u32_nctaid_z(a[0]),

            // --- Mov general ---
            InstType::MovU32 => self.mov_u32(a[0], a[1]),
            InstType::MovU32Imm => self.mov_u32_imm(a[0], a[1] as u32),
            InstType::MovU64 => self.mov_u64(a[0], a[1]),
            InstType::MovU64Imm => self.mov_u64_imm(a[0], a[1] as u64),
            InstType::MovF32 => self.mov_f32(a[0], a[1]),
            InstType::MovF32Imm => self.mov_f32_imm(a[0], f32::from_bits(a[1] as u32)),
            InstType::MovF32Bits => self.mov_f32_bits(a[0], a[1] as u32),
            InstType::MovB32FromF32 => self.mov_b32_from_f32(a[0], a[1]),
            InstType::MovF32FromB32 => self.mov_f32_from_b32(a[0], a[1]),

            // --- Arithmetic ---
            InstType::NegF32 => self.neg_f32(a[0], a[1]),
            InstType::AddS32 => self.add_s32(a[0], a[1], a[2]),
            InstType::AddS32Imm => self.add_s32_imm(a[0], a[1], a[2] as i32),
            InstType::AddS64 => self.add_s64(a[0], a[1], a[2]),
            InstType::AddF32 => self.add_f32(a[0], a[1], a[2]),
            InstType::AddF32Imm => self.add_f32_imm(a[0], a[1], f32::from_bits(a[2] as u32)),
            InstType::SubF32 => self.sub_f32(a[0], a[1], a[2]),
            InstType::MulF32 => self.mul_f32(a[0], a[1], a[2]),
            InstType::MulWideS32 => self.mul_wide_s32(a[0], a[1], a[2] as u64),
            InstType::MadLoS32 => self.mad_lo_s32(a[0], a[1], a[2], a[3]),
            InstType::FmaRnF32 => self.fma_rn_f32(a[0], a[1], a[2], a[3]),
            InstType::FmaRmF32 => self.fma_rm_f32(a[0], a[1], a[2], a[3]),
            InstType::ShlB32 => self.shl_b32(a[0], a[1], a[2] as u32),
            InstType::RcpRnF32 => self.rcp_rn_f32(a[0], a[1]),
            InstType::Ex2ApproxF32 => self.ex2_approx_ftz_f32(a[0], a[1]),

            // --- Conversion ---
            InstType::CvtaToGlobal => self.cvta_to_global_u64(a[0], a[1]),
            InstType::CvtSatF32F32 => self.cvt_sat_f32_f32(a[0], a[1]),

            // --- Memory ---
            InstType::LdGlobalU32 => self.ld_global_u32(a[0], a[1], mem, args),
            InstType::LdGlobalF32 => self.ld_global_f32(a[0], a[1], mem, args),
            InstType::LdGlobalNcF32 => self.ld_global_nc_f32(a[0], a[1], mem, args),
            InstType::StGlobalF32 => self.st_global_f32(a[0], a[1], mem, args),

            // --- Predicates ---
            InstType::SetpGeS32 => self.setp_ge_s32(a[0], a[1], a[2]),
            InstType::SetpGeS32Imm => self.setp_ge_s32_imm(a[0], a[1], a[2] as i32),
            InstType::SetpLtS32 => self.setp_lt_s32(a[0], a[1], a[2]),
            InstType::SetpLtS32Imm => self.setp_lt_s32_imm(a[0], a[1], a[2] as i32),
            InstType::OrPred => self.or_pred(a[0], a[1], a[2]),

            // --- Branches ---
            InstType::Bra => { self.bra(a[0] as u32); return; }
            InstType::BraIf => { self.bra_if(a[0], a[1] as u32); return; }
            InstType::BraIfNot => { self.bra_if_not(a[0], a[1] as u32); return; }

            // --- Control ---
            InstType::Ret => { self.pc = self.total_number_inst; return; }

            // --- Oth ---
            InstType::NoOp => { return; }
        }
        self.pc += 1;
    }

    fn execute_in_seq(&mut self, mem: &mut Memory, args: Vec<usize>) {
        let inst = self.inst_list[self.pc as usize].clone();
        self.execute_single_inst(inst, mem, args);
        // println!("done");
    }

    pub fn execute_all(&mut self, mem: &mut Memory, args: Vec<usize>) {
        self.pc = 0;                    // reset to start of instruction list
        self.branch_is_taken = false;   // clear branch flag

        while self.pc < self.total_number_inst {
            println!("{:?}", self.inst_list[self.pc as usize]);
            self.execute_in_seq(mem, args.clone());
        }
    }

    // Actually ISA insts -- Move and Loads

    fn load_param_u64(&mut self, dst: usize, addr: u64, mem: &Memory, args: Vec<usize>) {
        /*
        let mut bytes = [0u8; 8];
        println!("Loading param u64: addr = {}", addr);
        for i in 0..8 {
            let addr = MemoryAddress { address: (args[addr as usize] + i) as u64 };
            println!("Loading byte {}: addr = {}", i, addr.address);
            bytes[i] = mem.data.get(&addr).unwrap().value;
        }
        let result = u64::from_le_bytes(bytes);
        self.rd[dst] = result;
         */
        println!("{}", args[addr as usize] as u64);
        self.rd[dst] = args[addr as usize] as u64;
    }

    fn load_param_u32(&mut self, dst: usize, addr: u64, mem: &Memory, args: Vec<usize>) {
        /*
        let mut bytes = [0u8; 4];
        println!("Loading param u32: addr = {}", addr as usize);
        println!("{}", args[addr as usize]);
        for i in 0..4 {
            let addr = MemoryAddress { address: (args[addr as usize] + i) as u64 };
            bytes[i] = mem.data.get(&addr).unwrap().value;
        }
        let result = u32::from_le_bytes(bytes);
        self.r[dst] = result;
         */
        println!("{}", args[addr as usize] as u32);
        self.r[dst] = args[addr as usize] as u32;
    }

    fn load_param_f32(&mut self, dst: usize, addr: u64, mem: &Memory, args: Vec<usize>) {
        /*
        let mut bytes = [0u8; 4];
        for i in 0..4 {
            let addr = MemoryAddress { address: (args[addr as usize] + i) as u64 };
            bytes[i] = mem.data.get(&addr).unwrap().value;
        }
        let result = f32::from_le_bytes(bytes);
        self.f[dst] = result;
         */
        println!("{}", args[addr as usize] as f32);
        self.f[dst] = args[addr as usize] as f32;
    }

    fn mov_u32_tid_x(&mut self, dst: usize) {
        self.r[dst] = self.tid_x;
    }

    fn mov_u32_tid_y(&mut self, dst: usize) {
        self.r[dst] = self.tid_y;
    }

    fn mov_u32_tid_z(&mut self, dst: usize) {
        self.r[dst] = self.tid_z;
    }

    fn mov_u32_ctaid_x(&mut self, dst: usize) {
        self.r[dst] = self.ctaid_x;
    }

    fn mov_u32_ctaid_y(&mut self, dst: usize) {
        self.r[dst] = self.ctaid_y;
    }

    fn mov_u32_ctaid_z(&mut self, dst: usize) {
        self.r[dst] = self.ctaid_z;
    }

    fn mov_u32_ntid_x(&mut self, dst: usize) {
        self.r[dst] = self.ntid_x;
    }

    fn mov_u32_ntid_y(&mut self, dst: usize) {
        self.r[dst] = self.ntid_y;
    }

    fn mov_u32_ntid_z(&mut self, dst: usize) {
        self.r[dst] = self.ntid_z;
    }

    fn mov_u32_nctaid_x(&mut self, dst: usize) {
        self.r[dst] = self.nctaid_x;
    }

    fn mov_u32_nctaid_y(&mut self, dst: usize) {
        self.r[dst] = self.nctaid_y;
    }

    fn mov_u32_nctaid_z(&mut self, dst: usize) {
        self.r[dst] = self.nctaid_z;
    }

    fn mov_u32(&mut self, dst: usize, src: usize) {
        self.r[dst] = self.r[src];
    }

    fn mov_u32_imm(&mut self, dst: usize, imm: u32) {
        self.r[dst] = imm;
    }

    fn mov_u64(&mut self, dst: usize, src: usize) {
        self.rd[dst] = self.rd[src];
    }

    fn mov_u64_imm(&mut self, dst: usize, imm: u64) {
        self.rd[dst] = imm;
    }

    fn mov_f32(&mut self, dst: usize, src: usize) {
        self.f[dst] = self.f[src];
    }

    fn mov_f32_imm(&mut self, dst: usize, imm: f32) {
        self.f[dst] = imm;
    }

    fn mov_f32_bits(&mut self, dst: usize, bits: u32) {
        self.f[dst] = f32::from_bits(bits);
    }

    fn mov_b32_from_f32(&mut self, dst: usize, src: usize) {
        self.r[dst] = self.f[src].to_bits();
    }

    fn mov_f32_from_b32(&mut self, dst: usize, src: usize) {
        self.f[dst] = f32::from_bits(self.r[src]);
    }

    // Move and Loads -- End

    // Arithmetic Stuffs

    fn neg_f32(&mut self, dst: usize, src: usize) {
        self.f[dst] = -self.f[src];
    }

    fn add_s64(&mut self, dst: usize, a: usize, b: usize) {
        self.rd[dst] = self.rd[a].wrapping_add(self.rd[b]);
    }

    fn add_s32_imm(&mut self, dst: usize, a: usize, imm: i32) {
        self.r[dst] = (self.r[a] as i32).wrapping_add(imm) as u32;
    }

    fn add_s32(&mut self, dst: usize, a: usize, b: usize) {
        self.r[dst] = (self.r[a] as i32).wrapping_add(self.r[b] as i32) as u32;
    }

    fn add_f32(&mut self, dst: usize, a: usize, b: usize) {
        self.f[dst] = self.f[a] + self.f[b]
    }

    fn add_f32_imm(&mut self, dst: usize, a: usize, imm: f32) {
        self.f[dst] = self.f[a] + imm;
    }

    fn sub_f32(&mut self, dst: usize, a: usize, b: usize) {
        self.f[dst] = self.f[a] - self.f[b];
    }

    fn mul_f32(&mut self, dst: usize, a: usize, b: usize) {
        self.f[dst] = self.f[a] * self.f[b];
    }

    fn mad_lo_s32(&mut self, dst: usize, a: usize, b: usize, c: usize) {
        let result = (self.r[a] as i32).wrapping_mul(self.r[b] as i32).wrapping_add(self.r[c] as i32);
        self.r[dst] = result as u32;
    }

    fn mul_wide_s32(&mut self, dst: usize, a: usize, imm: u64) {
        let result = (self.r[a] as i32 as i64).wrapping_mul(imm as i64);
        self.rd[dst] = result as u64;
    }

    fn fma_rn_f32(&mut self, dst: usize, a: usize, b: usize, c: usize) {
        self.f[dst] = self.f[a] * self.f[b] + self.f[c];
    }

    fn fma_rm_f32(&mut self, dst: usize, a: usize, b: usize, c: usize) {
        self.f[dst] = self.f[a] * self.f[b] + self.f[c];
    }

    fn shl_b32(&mut self, dst: usize, src: usize, imm: u32) {
        self.r[dst] = self.r[src] << imm;
    }

    fn rcp_rn_f32(&mut self, dst: usize, src: usize) {
        self.f[dst] = 1.0f32 / self.f[src];
    }

    // Arithmetic Stuffs -- End

    // Branch and Conditionals

    fn setp_ge_s32(&mut self, dst: usize, a: usize, b: usize) {
        self.p[dst] = (self.r[a] as i32) >= (self.r[b] as i32);
    }

    fn setp_ge_s32_imm(&mut self, dst: usize, a: usize, imm: i32) {
        self.p[dst] = (self.p[a] as i32) >= imm;
    }

    fn setp_lt_s32(&mut self, dst: usize, a: usize, b: usize) {
        self.p[dst] = (self.r[a] as i32) < (self.r[b] as i32);
    }

    fn setp_lt_s32_imm(&mut self, dst: usize, a: usize, imm: i32) {
        self.p[dst] = (self.r[a] as i32) < imm;
    }

    fn or_pred(&mut self, dst: usize, a: usize, b: usize) {
        self.p[dst] = self.p[a] || self.p[b];
    }

    fn bra(&mut self, target_pc: u32) {
        self.pc = target_pc;
    }

    fn bra_if(&mut self, pred: usize, target_pc: u32) {
        if (self.p[pred]) {
            self.pc = target_pc;
        } else {
            self.pc += 1;
        }
    }

    fn bra_if_not(&mut self, pred: usize, target_pc: u32) {
        if !self.p[pred] {
            self.pc = target_pc;
        } else {
            self.pc += 1;
        }
    }

    // Branch and Conditionals -- End
    
    fn ld_global_u32(&mut self, dst: usize, addr_reg: usize, mem: &Memory, args: Vec<usize>) {
        let addr = self.rd[addr_reg];
        let mut bytes = [0u8; 4];
        for i in 0..4 {
            let addr = MemoryAddress { address: (addr as usize + i) as u64 };
            bytes[i] = mem.data.get(&addr).unwrap().value;
        }
        let result = u32::from_le_bytes(bytes);
        self.r[dst] = result;
        println!("LDGLOBALU32: res = {}", result);
    }

    fn ld_global_f32(&mut self, dst: usize, addr_reg: usize, mem: &Memory, args: Vec<usize>) {
        let addr = self.rd[addr_reg];
        let mut bytes = [0u8; 4];
        for i in 0..4 {
            let addr = MemoryAddress { address: (addr as usize + i) as u64 };
            bytes[i] = mem.data.get(&addr).unwrap().value;
        }
        let result = f32::from_le_bytes(bytes);
        self.f[dst] = result;
        println!("LDGLOBALF32: res = {}", result);
    }

    fn ld_global_nc_f32(&mut self, dst: usize, addr: usize, mem: &Memory, args: Vec<usize>) {
        self.ld_global_f32(dst, addr, mem, args);
    }

    fn st_global_f32(&mut self, addr_reg: usize, src: usize, mem: &mut Memory, args: Vec<usize>) {
        let addr = self.rd[addr_reg];
        let bytes = self.f[src].to_le_bytes();
        for i in 0..4 {
            let addr = MemoryAddress { address: (addr as usize + i) as u64 };
            if let Some(val) = mem.data.get_mut(&addr) {
                val.value = bytes[i];
            } else {
                // Shit Happened
            }
        }
    }

    // TBD, depends on how our global memory works. -- No clue how to categorize
    fn cvta_to_global_u64(&mut self, dst: usize, src: usize) {
        self.rd[dst] = self.rd[src];
    }

    fn cvt_sat_f32_f32(&mut self, dst: usize, src: usize) {
        self.f[dst] = self.f[src].clamp(0.0f32, 1.0f32);
    }

    fn ex2_approx_ftz_f32(&mut self, dst: usize, src: usize) {
        self.f[dst] = self.f[src].exp2();
        // flush denormals to zero
        if self.f[dst].is_subnormal() {
            self.f[dst] = 0.0f32;
        }
    }

}

/*
        match inst.inst_type {
            InstType::LdParamU64 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<u64>().unwrap();
                self.load_param_u64(dst, val);
            }
            InstType::LdParamU32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.load_param_u32(dst, val);
            }
            InstType::MovTidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_tid_x(dst);
            }
            InstType::MovTidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_tid_y(dst);
            }
            InstType::MovCtaidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ctaid_x(dst);
            }
            InstType::MovCtaidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ctaid_y(dst);
            }
            InstType::MovNtidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ntid_x(dst);
            }
            InstType::MovNtidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ntid_y(dst);
            }
            InstType::MadLoS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.mad_lo_s32(dst, a, b, c);
            }
            InstType::SetpGeS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.setp_ge_s32(dst, a, b);
            }
            InstType::OrPred => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.or_pred(dst, a, b);
            }
            InstType::BraIf => {
                let pred = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.bra_if(pred);
                if self.branch_is_taken {
                    self.pc = self.total_number_inst; // jump past end → exits while loop
                    return;
                }
            }
            InstType::CvtaToGlobal => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.cvta_to_global_u64(dst, src);
            }
            InstType::MulWideS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<u64>().unwrap();
                self.mul_wide_s32(dst, a, imm);
            }
            InstType::AddS64 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_s64(dst, a, b);
            }
            InstType::AddF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_f32(dst, a, b);
            }
            InstType::LdGlobalF32 => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let addr = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.ld_global_f32(dst, addr);
            }
            InstType::StGlobalF32 => {
                let addr = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src  = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.st_global_f32(addr, src);
            }
            InstType::Ret => {
                self.pc = self.total_number_inst;
            }
            InstType::SubF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.sub_f32(dst, a, b);
            }
            InstType::MulF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.mul_f32(dst, a, b);
            }
            InstType::LdParamF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<f32>().unwrap();
                self.load_param_f32(dst, val);
            }
            InstType::FmaRnF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.fma_rn_f32(dst, a, b, c);
            }
            InstType::AddS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_s32(dst, a, b);
            }
            InstType::AddS32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<i32>().unwrap();
                self.add_s32_imm(dst, a, imm);
            }
            InstType::SetpLtS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.setp_lt_s32(dst, a, b);
            }
            InstType::SetpLtS32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<i32>().unwrap();
                self.setp_lt_s32_imm(dst, a, imm);
            }
            InstType::MovU32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.mov_u32_imm(dst, imm);
            }
            InstType::MovF32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[1].downcast_ref::<f32>().unwrap();
                self.mov_f32_imm(dst, imm);
            }
            InstType::MovF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_f32(dst, src);
            }
            InstType::Bra => {
                let target = *inst.args[0].downcast_ref::<u32>().unwrap();
                self.bra(target);
                return; // don't increment pc
            }
            InstType::BraIfNot => {
                let pred   = *inst.args[0].downcast_ref::<usize>().unwrap();
                let target = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.bra_if_not(pred, target);
                if self.branch_is_taken {
                    return;
                }
            }
            InstType::NegF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.neg_f32(dst, src);
            }
            InstType::MovF32Bits => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let bits = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.mov_f32_bits(dst, bits);
            }
            InstType::FmaRmF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.fma_rm_f32(dst, a, b, c);
            }
            InstType::CvtSatF32F32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.cvt_sat_f32_f32(dst, src);
            }
            InstType::ShlB32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<u32>().unwrap();
                self.shl_b32(dst, src, imm);
            }
            InstType::MovB32FromF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_b32_from_f32(dst, src);
            }
            InstType::MovF32FromB32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_f32_from_b32(dst, src);
            }
            InstType::Ex2ApproxF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.ex2_approx_ftz_f32(dst, src);
            }
            InstType::RcpRnF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.rcp_rn_f32(dst, src);
            }
            InstType::AddF32Imm => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src  = *inst.args[1].downcast_ref::<usize>().unwrap();
                let bits = *inst.args[2].downcast_ref::<u32>().unwrap();
                self.add_f32_imm(dst, src, f32::from_bits(bits));
            }
        }
         */

/*
        let inst = &self.inst_list[self.pc as usize];
        match inst.inst_type {
            InstType::LdParamU64 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<u64>().unwrap();
                self.load_param_u64(dst, val);
            }
            InstType::LdParamU32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.load_param_u32(dst, val);
            }
            InstType::MovTidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_tid_x(dst);
            }
            InstType::MovTidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_tid_y(dst);
            }
            InstType::MovCtaidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ctaid_x(dst);
            }
            InstType::MovCtaidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ctaid_y(dst);
            }
            InstType::MovNtidX => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ntid_x(dst);
            }
            InstType::MovNtidY => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.mov_u32_ntid_y(dst);
            }
            InstType::MadLoS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.mad_lo_s32(dst, a, b, c);
            }
            InstType::SetpGeS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.setp_ge_s32(dst, a, b);
            }
            InstType::OrPred => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.or_pred(dst, a, b);
            }
            InstType::BraIf => {
                let pred = *inst.args[0].downcast_ref::<usize>().unwrap();
                self.bra_if(pred);
                if self.branch_is_taken {
                    return;
                }
            }
            InstType::CvtaToGlobal => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.cvta_to_global_u64(dst, src);
            }
            InstType::MulWideS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<u64>().unwrap();
                self.mul_wide_s32(dst, a, imm);
            }
            InstType::AddS64 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_s64(dst, a, b);
            }
            InstType::AddF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_f32(dst, a, b);
            }
            InstType::LdGlobalF32 => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let addr = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.ld_global_f32(dst, addr);
            }
            InstType::StGlobalF32 => {
                let addr = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src  = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.st_global_f32(addr, src);
            }
            InstType::Ret => {
                self.pc = self.total_number_inst;
            }
            InstType::SubF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.sub_f32(dst, a, b);
            }
            InstType::MulF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.mul_f32(dst, a, b);
            }
            InstType::LdParamF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let val = *inst.args[1].downcast_ref::<f32>().unwrap();
                self.load_param_f32(dst, val);
            }
            InstType::FmaRnF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.fma_rn_f32(dst, a, b, c);
            }
            InstType::AddS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.add_s32(dst, a, b);
            }
            InstType::AddS32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<i32>().unwrap();
                self.add_s32_imm(dst, a, imm);
            }
            InstType::SetpLtS32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                self.setp_lt_s32(dst, a, b);
            }
            InstType::SetpLtS32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<i32>().unwrap();
                self.setp_lt_s32_imm(dst, a, imm);
            }
            InstType::MovU32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.mov_u32_imm(dst, imm);
            }
            InstType::MovF32Imm => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[1].downcast_ref::<f32>().unwrap();
                self.mov_f32_imm(dst, imm);
            }
            InstType::MovF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_f32(dst, src);
            }
            InstType::Bra => {
                let target = *inst.args[0].downcast_ref::<u32>().unwrap();
                self.bra(target);
                return; // don't increment pc
            }
            InstType::BraIfNot => {
                let pred   = *inst.args[0].downcast_ref::<usize>().unwrap();
                let target = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.bra_if_not(pred, target);
                if self.branch_is_taken {
                    return;
                }
            }
            InstType::NegF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.neg_f32(dst, src);
            }
            InstType::MovF32Bits => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let bits = *inst.args[1].downcast_ref::<u32>().unwrap();
                self.mov_f32_bits(dst, bits);
            }
            InstType::FmaRmF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let a   = *inst.args[1].downcast_ref::<usize>().unwrap();
                let b   = *inst.args[2].downcast_ref::<usize>().unwrap();
                let c   = *inst.args[3].downcast_ref::<usize>().unwrap();
                self.fma_rm_f32(dst, a, b, c);
            }
            InstType::CvtSatF32F32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.cvt_sat_f32_f32(dst, src);
            }
            InstType::ShlB32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                let imm = *inst.args[2].downcast_ref::<u32>().unwrap();
                self.shl_b32(dst, src, imm);
            }
            InstType::MovB32FromF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_b32_from_f32(dst, src);
            }
            InstType::MovF32FromB32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.mov_f32_from_b32(dst, src);
            }
            InstType::Ex2ApproxF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.ex2_approx_ftz_f32(dst, src);
            }
            InstType::RcpRnF32 => {
                let dst = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src = *inst.args[1].downcast_ref::<usize>().unwrap();
                self.rcp_rn_f32(dst, src);
            }
            InstType::AddF32Imm => {
                let dst  = *inst.args[0].downcast_ref::<usize>().unwrap();
                let src  = *inst.args[1].downcast_ref::<usize>().unwrap();
                let bits = *inst.args[2].downcast_ref::<u32>().unwrap();
                self.add_f32_imm(dst, src, f32::from_bits(bits));
            }
        }
        self.pc += 1;
         */