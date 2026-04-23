//! PTX parser for the GPU emulator project.
//!
//! Two input formats are supported and auto-detected:
//!
//! 1. **PTX source** — full parsing pipeline:
//!        PTX text
//!          -> [lexer: logos]           -> Tokens
//!          -> [hand-written parser]    -> Vec<RawInstruction>  (intermediate representation)
//!          -> [lowering]               -> label resolution + InstType selection
//!          -> ParsedKernel { instructions: Vec<inst_info>, ... }
//!
//! 2. **C function signature** — schema-only parse:
//!        C text
//!          -> [c_signature parser]     -> ParsedKernel { instructions: vec![], ... }
//!
//! Public API is the `parse` function plus the output types re-exported
//! below. Everything else is internal to this module.
//!
//! File map:
//!   output.rs      -- ParsedKernel, ParamInfo, PtxType (final output)
//!   error.rs       -- ParseError
//!   ir.rs          -- RawInstruction, RawOperand, etc. (intermediate representation)
//!   lexer.rs       -- logos token definitions
//!   parser.rs      -- tokens -> Vec<RawInstruction>
//!   lowering.rs    -- Vec<RawInstruction> -> Vec<inst_info>
//!   c_signature.rs -- parser for C function signatures (secondary input format)

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

pub use error::ParseError;
pub use output::{ParamInfo, ParsedKernel, PtxType};

pub use ir::{inst_info, InstType};

/// Parse a PTX source string or a C function into ParsedKernel
pub fn parse(input: &str) -> Result<ParsedKernel, ParseError> {
    if c_signature::looks_like_ptx(input) {
        let tokens = lexer::tokenize(input);
        let parse_out = parser::parse_tokens(tokens)?;
        lowering::lower(parse_out)
    } else {
        c_signature::parse_c_signature(input)
    }
}