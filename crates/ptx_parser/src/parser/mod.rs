
mod c_signature;
mod ir;
mod lexer;
mod lowering;
mod parser;

#[cfg(test)]
mod print_tests;

pub mod error;
pub mod output;


pub use error::ParseError;
pub use output::{ParamInfo, ParsedKernel, PtxType};

pub use ir::{inst_info, InstType};

///parse PTX str to parseKernal
pub fn parse(ptx: &str) -> Result<ParsedKernel, ParseError> {
    let tokens = lexer::tokenize(ptx);
    let parse_out = parser::parse_tokens(tokens)?;
    lowering::lower(parse_out)
}

//parse c sig str
pub fn parse_c_signature(sig: &str) -> Result<ParsedKernel, ParseError> {
    c_signature::parse_c_signature(sig)
}