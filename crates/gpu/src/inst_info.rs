use std::any::Any;
use std::ffi::c_void;
use memory::MemoryAddress;
use crate::basegpu::GPU0;
use crate::inst_type::InstType;

#[derive(Default, Clone)]
pub struct inst_info {
    pub(crate) inst_type: InstType,
    pub(crate) args:Vec<usize>,
}

pub fn make_inst(inst_type: InstType, args: Vec<usize>) -> inst_info {
    inst_info {
        inst_type,
        args,
    }
}