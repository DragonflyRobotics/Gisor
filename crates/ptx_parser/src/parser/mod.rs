//! PTX parser for the GPU emulator project.
//!
//! Two parser entry points are provided:
//!
//! 1. `parse(ptx: &str)` -> `ParsedKernel`
//!        Parses PTX source:
//!          PTX text
//!            -> [lexer: logos]           -> Tokens
//!            -> [hand-written parser]    -> Vec<RawInstruction>  (intermediate representation)
//!            -> [lowering]               -> label resolution + InstType selection
//!            -> ParsedKernel { name, params, instructions }
//!
//! 2. `parse_c_signature(sig: &str)` -> `ParsedSignature`
//!        Parses a C/C++ function signature (full or demangled):
//!          C text
//!            -> [c_signature parser]     -> ParsedSignature { name, params with pointer_levels }
//!
//! Public API is these two functions plus the output types re-exported
//! below. Everything else is internal to this module.
//!
//! File map:
//!   output.rs      -- ParsedKernel, ParsedSignature, ParamInfo, SignatureParam, PtxType
//!   error.rs       -- ParseError
//!   ir.rs          -- RawInstruction, RawOperand, etc. (intermediate representation)
//!   lexer.rs       -- logos token definitions
//!   parser.rs      -- tokens -> Vec<RawInstruction>
//!   lowering.rs    -- Vec<RawInstruction> -> Vec<inst_info>
//!   c_signature.rs -- parser for C function signatures

// `ir` holds intermediate types (RawInstruction, RawOperand, etc.) used
// between the parser and lowering phases. These are implementation details,
// not part of the public API, so the module is private.
mod c_signature;
mod ir;
mod lexer;
mod lowering;
mod parser;

#[cfg(test)]
mod print_tests;

pub mod error;
pub mod output;

// Re-export the public surface.
pub use error::ParseError;
pub use output::{ParamInfo, ParsedKernel, ParsedSignature, PtxType, SignatureParam};

// Re-export Zekai's instruction types at the parser's public API so
// callers can write `ptx_parser::inst_info` / `ptx_parser::InstType`.
pub use ir::{inst_info, InstType};

/// Parse PTX source text into a fully-populated `ParsedKernel`.
///
/// Use this when Krishna has the compiled PTX body of a kernel. The
/// returned struct has `name`, `params`, and `instructions` populated.
pub fn parse(ptx: &str) -> Result<ParsedKernel, ParseError> {
    let tokens = lexer::tokenize(ptx);
    let parse_out = parser::parse_tokens(tokens)?;
    lowering::lower(parse_out)
}

/// Parse a C/C++ function signature into a `ParsedSignature`.
///
/// Accepts two styles:
///
///   // full signature (from source):
///   void addKernel(float* A, float* B, float* C, int N)
///
///   // demangled-style signature (from `cpp_demangle` or c++filt):
///   addKernel(float*, float*, float*, int)
///
/// Parameter names are optional in both styles; a missing name becomes
/// an empty string. Each `SignatureParam` carries both the dereferenced
/// type and the number of pointer levels, so `float*` parses as
/// `{ ptx_type: F32, pointer_levels: 1 }`.
pub fn parse_c_signature(sig: &str) -> Result<ParsedSignature, ParseError> {
    c_signature::parse_c_signature(sig)
}