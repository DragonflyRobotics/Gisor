//! Hand-written parser for PTX.
//!
//! Consumes a `Vec<(Token, Span)>` from the lexer and produces a partially-
//! built `ParsedKernel` plus a `Vec<RawInstruction>` that the lowering phase
//! will later convert to `Vec<inst_info>`.
//!
//! Design: simple recursive-descent. The `Parser` struct owns the token
//! vector and a cursor index. Helper methods (`peek`, `advance`, `expect`)
//! handle the common look/consume patterns. Each PTX construct has a
//! dedicated `parse_*` method.
//!
//! This module does NO semantic work: it doesn't pick `InstType` variants,
//! doesn't resolve labels to PCs, doesn't encode operands to `usize`. All
//! of that is the lowering phase's job. The parser's only responsibility
//! is "tokens came in, faithfully structured data came out."

use crate::parser::ir::{
    ImmediateValue, PredGuard, RawInstruction, RawOperand, RegBank, SpecialReg,
};
use crate::parser::error::ParseError;
use crate::parser::lexer::{Span, Token};
use crate::parser::output::{ParamInfo, PtxType};

/// Result of the parse phase. This is not the final `ParsedKernel` (the
/// `instructions` field still holds raw instructions, not `inst_info`) —
/// the lowering phase consumes this struct and produces the final output.
#[derive(Debug, Clone, Default)]
pub struct ParseOutput {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub raw_instructions: Vec<RawInstruction>,
}

/// Public entry point.
pub fn parse_tokens(tokens: Vec<(Token, Span)>) -> Result<ParseOutput, ParseError> {
    let mut p = Parser::new(tokens);
    p.parse_top_level()
}

// ----------------------------------------------------------------------------
// Parser state
// ----------------------------------------------------------------------------

struct Parser {
    tokens: Vec<(Token, Span)>,
    cursor: usize,
    /// Current 1-based source line number, advanced on each `Token::Newline`.
    line: usize,
}

impl Parser {
    fn new(tokens: Vec<(Token, Span)>) -> Self {
        Self {
            tokens,
            cursor: 0,
            line: 1,
        }
    }

    // -- primitive cursor operations ---------------------------------------

    /// Look at the current token without consuming it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor).map(|(t, _)| t)
    }

    /// Look at the token `n` positions ahead without consuming it. Skips
    /// newlines for the purpose of lookahead, because newlines are not
    /// structurally significant in PTX.
    fn peek_nonws(&self, n: usize) -> Option<&Token> {
        let mut seen = 0;
        for (t, _) in self.tokens.iter().skip(self.cursor) {
            if matches!(t, Token::Newline) {
                continue;
            }
            if seen == n {
                return Some(t);
            }
            seen += 1;
        }
        None
    }

    /// Consume and return the current token. Updates `self.line` if it's
    /// a newline.
    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.cursor)?.0.clone();
        self.cursor += 1;
        if matches!(tok, Token::Newline) {
            self.line += 1;
        }
        Some(tok)
    }

    /// Consume any run of newlines. PTX instructions are terminated by `;`,
    /// not newlines, so the parser treats newlines as skippable whitespace
    /// everywhere except for line-number tracking.
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }
    }

    /// Consume the current token if it matches `expected`. Error otherwise.
    fn expect(&mut self, expected: &Token, what: &str) -> Result<(), ParseError> {
        self.skip_newlines();
        match self.peek() {
            Some(t) if std::mem::discriminant(t) == std::mem::discriminant(expected) => {
                self.advance();
                Ok(())
            }
            Some(t) => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: what.to_string(),
                found: format!("{t:?}"),
            }),
            None => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: what.to_string(),
                found: "end of input".to_string(),
            }),
        }
    }

    /// Consume an identifier and return its string, erroring if the current
    /// token is something else.
    fn expect_ident(&mut self) -> Result<String, ParseError> {
        self.skip_newlines();
        match self.peek() {
            Some(Token::Ident(_)) => {
                if let Some(Token::Ident(s)) = self.advance() {
                    Ok(s)
                } else {
                    unreachable!()
                }
            }
            Some(t) => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "identifier".to_string(),
                found: format!("{t:?}"),
            }),
            None => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "identifier".to_string(),
                found: "end of input".to_string(),
            }),
        }
    }

    /// Consume an identifier IF it matches `name`. Otherwise do nothing and
    /// return false. Useful for directives where you want to check
    /// "is the next ident `version`?" without erroring.
    fn eat_ident(&mut self, name: &str) -> bool {
        self.skip_newlines();
        if let Some(Token::Ident(s)) = self.peek() {
            if s == name {
                self.advance();
                return true;
            }
        }
        false
    }

    /// Consume a specific punctuation token if present. Returns whether it
    /// was consumed.
    fn eat(&mut self, tok: &Token) -> bool {
        self.skip_newlines();
        if let Some(t) = self.peek() {
            if std::mem::discriminant(t) == std::mem::discriminant(tok) {
                self.advance();
                return true;
            }
        }
        false
    }

    // -- top-level structure -----------------------------------------------

    /// Parse a whole PTX file: optional header directives, then exactly one
    /// `.visible .entry` kernel definition.
    fn parse_top_level(&mut self) -> Result<ParseOutput, ParseError> {
        // Skip header directives: .version, .target, .address_size, and any
        // line-comment-like stray `//.globl ...` which the lexer has already
        // removed as a comment. We just consume tokens until we find
        // `.visible` or `.entry`.
        self.skip_header_directives()?;

        // Parse the kernel entry.
        let (name, params) = self.parse_entry_header()?;
        self.expect(&Token::LBrace, "`{` to start kernel body")?;

        // Consume `.reg` declarations; we don't retain their counts since
        // Zekai's executor uses fixed 256-entry register files.
        self.parse_reg_decls()?;
        let raw_instructions = self.parse_instruction_list()?;

        self.expect(&Token::RBrace, "`}` to close kernel body")?;

        Ok(ParseOutput {
            name,
            params,
            raw_instructions,
        })
    }

    /// Skip file-level directives that we don't care about. Consumes tokens
    /// until it sees `.visible` or `.entry`. Each directive is a `Dot`
    /// followed by an identifier and some arguments terminated by a newline
    /// or another `Dot`.
    fn skip_header_directives(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_newlines();
            match self.peek() {
                // End of useful content before .entry — error.
                None => return Err(ParseError::MissingEntry),

                // A directive. Peek at the name.
                Some(Token::Dot) => {
                    // Look at the identifier after the dot.
                    let name = match self.peek_nonws(1) {
                        Some(Token::Ident(s)) => s.clone(),
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                line: self.line,
                                expected: "directive name after `.`".to_string(),
                                found: "something else".to_string(),
                            })
                        }
                    };

                    // `.visible` and `.entry` mark the start of the kernel;
                    // stop skipping and hand off to `parse_entry_header`.
                    if name == "visible" || name == "entry" {
                        return Ok(());
                    }

                    // Skip an unknown header directive entirely: consume
                    // tokens until we hit a newline. This handles
                    // `.version 9.1`, `.target sm_75`, `.address_size 64`.
                    while !matches!(self.peek(), Some(Token::Newline) | None) {
                        self.advance();
                    }
                }

                // Anything else at top level is unexpected.
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        line: self.line,
                        expected: "file-level directive".to_string(),
                        found: format!("{t:?}"),
                    })
                }
            }
        }
    }

    /// Parse `.visible .entry <name>( .param <ty> <name>, ... )`.
    /// Returns (kernel_name, params).
    fn parse_entry_header(&mut self) -> Result<(String, Vec<ParamInfo>), ParseError> {
        self.skip_newlines();

        // Optional `.visible` prefix — some PTX omits it.
        self.expect(&Token::Dot, "`.` before entry directive")?;
        if self.eat_ident("visible") {
            // Move on to the next `.`.
            self.expect(&Token::Dot, "`.` before `entry`")?;
        }
        if !self.eat_ident("entry") {
            return Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "`entry`".to_string(),
                found: "something else".to_string(),
            });
        }

        // Kernel name.
        let name = self.expect_ident()?;

        // Parameter list.
        self.expect(&Token::LParen, "`(` before parameter list")?;
        let params = self.parse_param_list()?;
        self.expect(&Token::RParen, "`)` after parameter list")?;

        Ok((name, params))
    }

    fn parse_param_list(&mut self) -> Result<Vec<ParamInfo>, ParseError> {
        let mut params = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Some(Token::RParen) | None => break, // end of list
                Some(Token::Comma) => {
                    self.advance();
                    continue;
                }
                Some(Token::Dot) => {
                    // `.param .<type> <name>`
                    self.advance(); // consume first `.`
                    if !self.eat_ident("param") {
                        return Err(ParseError::UnexpectedToken {
                            line: self.line,
                            expected: "`param`".to_string(),
                            found: "something else".to_string(),
                        });
                    }
                    self.expect(&Token::Dot, "`.` before param type")?;
                    let ty_str = self.expect_ident()?;
                    let ptx_type = parse_ptx_type(&ty_str, self.line)?;
                    let param_name = self.expect_ident()?;
                    params.push(ParamInfo {
                        name: param_name,
                        ptx_type,
                    });
                }
                Some(t) => {
                    return Err(ParseError::UnexpectedToken {
                        line: self.line,
                        expected: "`.param` or `)`".to_string(),
                        found: format!("{t:?}"),
                    })
                }
            }
        }
        Ok(params)
    }

    /// Parse `.reg .<type> %<bank><count>;` lines until a non-`.reg` token.
    ///
    /// Example PTX:
    ///   .reg .pred  %p<2>;
    ///   .reg .f32   %f<4>;
    ///   .reg .b32   %r<6>;
    ///   .reg .b64   %rd<11>;
    ///
    /// Note that `<` and `>` in PTX source are literal syntax here, not
    /// comparison operators. Our lexer doesn't emit those as special tokens,
    /// so they'll show up as... well, they shouldn't, because our lexer has
    /// no tokens for `<` or `>`. The logos lexer would fail on these chars.
    /// We handle this by scanning raw bytes until `;`, since we don't
    /// actually need to parse the count — the register bank name is enough.
    fn parse_reg_decls(&mut self) -> Result<(), ParseError> {
        loop {
            self.skip_newlines();
            // Look ahead: if next non-newline is `.` followed by `reg`, continue.
            // Otherwise, we're past the reg decls.
            let is_reg = matches!(self.peek(), Some(Token::Dot))
                && matches!(self.peek_nonws(1), Some(Token::Ident(s)) if s == "reg");
            if !is_reg {
                break;
            }

            self.advance(); // `.`
            self.advance(); // `reg`
            self.expect(&Token::Dot, "`.` before reg type")?;
            let ty = self.expect_ident()?;
            // `%`
            self.expect(&Token::Percent, "`%` before register bank name")?;
            let bank_name = self.expect_ident()?;

            // The remainder looks like `<N>;`. Our lexer doesn't tokenize `<`
            // or `>`, so they get silently dropped and the count appears as
            // a bare `IntDec`. Accept the optional integer and then `;`.
            if matches!(self.peek(), Some(Token::IntDec(_))) {
                self.advance();
            }

            self.expect(&Token::Semicolon, "`;` after reg declaration")?;

            // Validate the bank name but discard everything else. The type
            // tag (`ty`) is not used — Zekai's executor doesn't need it.
            let _ = ty;
            match bank_name.as_str() {
                "p" | "r" | "rd" | "f" => {}
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        line: self.line,
                        expected: "register bank (p, r, rd, f)".to_string(),
                        found: bank_name,
                    })
                }
            }
        }

        Ok(())
    }

    // -- instruction list --------------------------------------------------

    fn parse_instruction_list(&mut self) -> Result<Vec<RawInstruction>, ParseError> {
        let mut insts = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Some(Token::RBrace) | None => break,
                Some(Token::Label(_)) => {
                    // Label line: `$L__BB0_2:`. We turn this into a
                    // synthetic "label" pseudo-instruction so the lowering
                    // phase can build its label map in a single pass.
                    //
                    // We represent labels as a RawInstruction with mnemonic
                    // ".label" and a single Label operand. Lowering
                    // recognizes this and does NOT emit an inst_info for it
                    // — it just records the current PC in the label map.
                    let label = if let Some(Token::Label(s)) = self.advance() {
                        s
                    } else {
                        unreachable!()
                    };
                    self.expect(&Token::Colon, "`:` after label")?;
                    insts.push(RawInstruction {
                        predicate_guard: None,
                        mnemonic: ".label".to_string(),
                        modifiers: Vec::new(),
                        operands: vec![RawOperand::Label(label)],
                        line: self.line,
                    });
                }
                Some(_) => {
                    let inst = self.parse_instruction()?;
                    insts.push(inst);
                }
            }
        }
        Ok(insts)
    }

    fn parse_instruction(&mut self) -> Result<RawInstruction, ParseError> {
        self.skip_newlines();
        let start_line = self.line;

        // Optional predicate guard: `@%p1` or `@!%p1`.
        let predicate_guard = match self.peek() {
            Some(Token::At) => {
                self.advance();
                self.expect(&Token::Percent, "`%` after `@`")?;
                let name = self.expect_ident()?;
                let idx = parse_pred_index(&name, self.line)?;
                Some(PredGuard {
                    reg: idx,
                    negated: false,
                })
            }
            Some(Token::AtNot) => {
                self.advance();
                self.expect(&Token::Percent, "`%` after `@!`")?;
                let name = self.expect_ident()?;
                let idx = parse_pred_index(&name, self.line)?;
                Some(PredGuard {
                    reg: idx,
                    negated: true,
                })
            }
            _ => None,
        };

        // Mnemonic.
        let mnemonic = self.expect_ident()?;

        // Zero or more dotted modifiers.
        let mut modifiers = Vec::new();
        while matches!(self.peek(), Some(Token::Dot)) {
            self.advance();
            modifiers.push(self.expect_ident()?);
        }

        // Operands (comma-separated, terminated by `;`).
        let mut operands = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Some(Token::Semicolon) => {
                    self.advance();
                    break;
                }
                Some(Token::Comma) => {
                    self.advance();
                    continue;
                }
                None => {
                    return Err(ParseError::UnexpectedToken {
                        line: self.line,
                        expected: "`;` to end instruction".to_string(),
                        found: "end of input".to_string(),
                    })
                }
                _ => {
                    operands.push(self.parse_operand()?);
                }
            }
        }

        Ok(RawInstruction {
            predicate_guard,
            mnemonic,
            modifiers,
            operands,
            line: start_line,
        })
    }

    fn parse_operand(&mut self) -> Result<RawOperand, ParseError> {
        self.skip_newlines();
        match self.peek() {
            // Register: `%<bank><index>` or special register `%tid.x`.
            Some(Token::Percent) => {
                self.advance();
                let name = self.expect_ident()?;

                // Could be a special register: `%tid`, `%ntid`, `%ctaid`,
                // or `%nctaid`, followed by `.x`, `.y`, or `.z`.
                if matches!(name.as_str(), "tid" | "ntid" | "ctaid" | "nctaid")
                    && matches!(self.peek(), Some(Token::Dot))
                {
                    self.advance(); // `.`
                    let axis = self.expect_ident()?;
                    let sr = match (name.as_str(), axis.as_str()) {
                        ("tid", "x") => SpecialReg::TidX,
                        ("tid", "y") => SpecialReg::TidY,
                        ("tid", "z") => SpecialReg::TidZ,
                        ("ntid", "x") => SpecialReg::NtidX,
                        ("ntid", "y") => SpecialReg::NtidY,
                        ("ntid", "z") => SpecialReg::NtidZ,
                        ("ctaid", "x") => SpecialReg::CtaidX,
                        ("ctaid", "y") => SpecialReg::CtaidY,
                        ("ctaid", "z") => SpecialReg::CtaidZ,
                        ("nctaid", "x") => SpecialReg::NctaidX,
                        ("nctaid", "y") => SpecialReg::NctaidY,
                        ("nctaid", "z") => SpecialReg::NctaidZ,
                        _ => {
                            return Err(ParseError::UnexpectedToken {
                                line: self.line,
                                expected: "special register axis (.x, .y, or .z)".to_string(),
                                found: axis,
                            })
                        }
                    };
                    return Ok(RawOperand::SpecialReg(sr));
                }

                // Regular register: split `rd1` into bank `Rd` and index `1`.
                let (bank, index) = parse_register_name(&name, self.line)?;
                Ok(RawOperand::Register { bank, index })
            }

            // Memory reference: `[%rd8]` or `[param_name]`.
            Some(Token::LBracket) => {
                self.advance();
                let inner = self.parse_operand()?;
                self.expect(&Token::RBracket, "`]` to close memory reference")?;
                Ok(RawOperand::MemoryRef(Box::new(inner)))
            }

            // Label: `$L__BB0_2` used as a branch target.
            Some(Token::Label(_)) => {
                if let Some(Token::Label(s)) = self.advance() {
                    Ok(RawOperand::Label(s))
                } else {
                    unreachable!()
                }
            }

            // Integer immediate (decimal or hex).
            Some(Token::IntDec(_)) => {
                if let Some(Token::IntDec(v)) = self.advance() {
                    Ok(RawOperand::Immediate(ImmediateValue::Int(v)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::IntHex(_)) => {
                if let Some(Token::IntHex(v)) = self.advance() {
                    Ok(RawOperand::Immediate(ImmediateValue::Int(v)))
                } else {
                    unreachable!()
                }
            }

            // Float-bits immediate: `0f3F800000`.
            Some(Token::FloatBits(_)) => {
                if let Some(Token::FloatBits(v)) = self.advance() {
                    Ok(RawOperand::Immediate(ImmediateValue::F32Bits(v)))
                } else {
                    unreachable!()
                }
            }

            // Bare identifier: kernel parameter name inside `[...]`.
            Some(Token::Ident(_)) => {
                if let Some(Token::Ident(s)) = self.advance() {
                    Ok(RawOperand::Identifier(s))
                } else {
                    unreachable!()
                }
            }

            Some(t) => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "operand".to_string(),
                found: format!("{t:?}"),
            }),
            None => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "operand".to_string(),
                found: "end of input".to_string(),
            }),
        }
    }
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

/// Parse a PTX type identifier like "u64" or "f32" into a `PtxType`.
fn parse_ptx_type(s: &str, line: usize) -> Result<PtxType, ParseError> {
    Ok(match s {
        "u32" => PtxType::U32,
        "u64" => PtxType::U64,
        "s32" => PtxType::S32,
        "s64" => PtxType::S64,
        "f32" => PtxType::F32,
        "b32" => PtxType::B32,
        "b64" => PtxType::B64,
        "pred" => PtxType::Pred,
        _ => {
            return Err(ParseError::UnexpectedToken {
                line,
                expected: "PTX type (u32, u64, s32, s64, f32, b32, b64, pred)".to_string(),
                found: s.to_string(),
            })
        }
    })
}

/// Split a register identifier like "rd1" or "r4" into its bank and index.
///
/// Strategy: the bank prefix is the longest leading run of lowercase letters,
/// the index is the remaining digits.
fn parse_register_name(s: &str, line: usize) -> Result<(RegBank, u32), ParseError> {
    let split = s
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| ParseError::UnexpectedToken {
            line,
            expected: "register name with numeric index".to_string(),
            found: s.to_string(),
        })?;
    let (prefix, digits) = s.split_at(split);
    let bank = match prefix {
        "p" => RegBank::P,
        "r" => RegBank::R,
        "rd" => RegBank::Rd,
        "f" => RegBank::F,
        _ => {
            return Err(ParseError::UnexpectedToken {
                line,
                expected: "register bank prefix (p, r, rd, f)".to_string(),
                found: prefix.to_string(),
            })
        }
    };
    let index: u32 = digits.parse().map_err(|_| ParseError::UnexpectedToken {
        line,
        expected: "register index (non-negative integer)".to_string(),
        found: digits.to_string(),
    })?;
    if index > 255 {
        return Err(ParseError::RegisterOutOfRange { line, bank, index });
    }
    Ok((bank, index))
}

/// Parse a predicate register name ("p1" → 1).
fn parse_pred_index(s: &str, line: usize) -> Result<u32, ParseError> {
    let (bank, index) = parse_register_name(s, line)?;
    if bank != RegBank::P {
        return Err(ParseError::UnexpectedToken {
            line,
            expected: "predicate register (%p...)".to_string(),
            found: s.to_string(),
        });
    }
    Ok(index)
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------
//
// These tests exercise the parser against hand-written PTX snippets and
// verify the resulting RawInstruction list. Lowering (the next phase) is
// not tested here.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::lexer::tokenize;

    fn parse(input: &str) -> ParseOutput {
        parse_tokens(tokenize(input)).expect("parse should succeed")
    }

    #[test]
    fn empty_kernel() {
        let src = r#"
            .version 9.1
            .target sm_75
            .address_size 64
            .visible .entry foo()
            {
                ret;
            }
        "#;
        let out = parse(src);
        assert_eq!(out.name, "foo");
        assert!(out.params.is_empty());
        assert_eq!(out.raw_instructions.len(), 1);
        assert_eq!(out.raw_instructions[0].mnemonic, "ret");
    }

    #[test]
    fn kernel_with_params() {
        let src = r#"
            .visible .entry bar(
                .param .u64 bar_param_0,
                .param .u32 bar_param_1
            )
            {
                ret;
            }
        "#;
        let out = parse(src);
        assert_eq!(out.params.len(), 2);
        assert_eq!(out.params[0].name, "bar_param_0");
        assert_eq!(out.params[0].ptx_type, PtxType::U64);
        assert_eq!(out.params[1].name, "bar_param_1");
        assert_eq!(out.params[1].ptx_type, PtxType::U32);
    }

    #[test]
    fn instruction_with_registers() {
        // `mad.lo.s32 %r1, %r5, %r4, %r3;`
        let src = r#"
            .visible .entry k()
            {
                mad.lo.s32 %r1, %r5, %r4, %r3;
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert_eq!(inst.mnemonic, "mad");
        assert_eq!(inst.modifiers, vec!["lo", "s32"]);
        assert_eq!(inst.operands.len(), 4);
        for (i, expected_idx) in [1, 5, 4, 3].iter().enumerate() {
            match &inst.operands[i] {
                RawOperand::Register { bank, index } => {
                    assert_eq!(*bank, RegBank::R);
                    assert_eq!(*index, *expected_idx);
                }
                other => panic!("expected register, got {other:?}"),
            }
        }
    }

    #[test]
    fn instruction_with_immediate() {
        let src = r#"
            .visible .entry k()
            {
                mul.wide.s32 %rd3, %r10, 4;
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert_eq!(inst.mnemonic, "mul");
        assert_eq!(inst.modifiers, vec!["wide", "s32"]);
        assert_eq!(inst.operands.len(), 3);
        assert!(matches!(
            &inst.operands[2],
            RawOperand::Immediate(ImmediateValue::Int(4))
        ));
    }

    #[test]
    fn instruction_with_memory_ref() {
        let src = r#"
            .visible .entry k()
            {
                ld.global.f32 %f1, [%rd8];
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert_eq!(inst.operands.len(), 2);
        match &inst.operands[1] {
            RawOperand::MemoryRef(inner) => match &**inner {
                RawOperand::Register { bank, index } => {
                    assert_eq!(*bank, RegBank::Rd);
                    assert_eq!(*index, 8);
                }
                other => panic!("expected register inside memref, got {other:?}"),
            },
            other => panic!("expected memref, got {other:?}"),
        }
    }

    #[test]
    fn instruction_with_param_ref() {
        let src = r#"
            .visible .entry k(
                .param .u64 k_param_0
            )
            {
                ld.param.u64 %rd1, [k_param_0];
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        match &inst.operands[1] {
            RawOperand::MemoryRef(inner) => match &**inner {
                RawOperand::Identifier(s) => assert_eq!(s, "k_param_0"),
                other => panic!("expected identifier, got {other:?}"),
            },
            other => panic!("expected memref, got {other:?}"),
        }
    }

    #[test]
    fn special_register_operand() {
        let src = r#"
            .visible .entry k()
            {
                mov.u32 %r3, %tid.x;
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert!(matches!(
            &inst.operands[1],
            RawOperand::SpecialReg(SpecialReg::TidX)
        ));
    }

    #[test]
    fn predicate_guarded_branch() {
        let src = r#"
            .visible .entry k()
            {
                @%p1 bra target;
            $L__BB0_2:
                ret;
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert_eq!(inst.mnemonic, "bra");
        let guard = inst.predicate_guard.expect("should have guard");
        assert_eq!(guard.reg, 1);
        assert!(!guard.negated);
    }

    #[test]
    fn label_becomes_synthetic_instruction() {
        let src = r#"
            .visible .entry k()
            {
                ret;
            $L__BB0_2:
                ret;
            }
        "#;
        let out = parse(src);
        assert_eq!(out.raw_instructions.len(), 3);
        assert_eq!(out.raw_instructions[0].mnemonic, "ret");
        assert_eq!(out.raw_instructions[1].mnemonic, ".label");
        match &out.raw_instructions[1].operands[0] {
            RawOperand::Label(s) => assert_eq!(s, "$L__BB0_2"),
            other => panic!("expected label, got {other:?}"),
        }
        assert_eq!(out.raw_instructions[2].mnemonic, "ret");
    }

    #[test]
    fn float_immediate() {
        let src = r#"
            .visible .entry k()
            {
                mov.f32 %f1, 0f3F800000;
            }
        "#;
        let out = parse(src);
        let inst = &out.raw_instructions[0];
        assert!(matches!(
            &inst.operands[1],
            RawOperand::Immediate(ImmediateValue::F32Bits(0x3F800000))
        ));
    }

    #[test]
    fn full_add_kernel() {
        // The entire addKernel example from the project notes.
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
        let out = parse(src);
        assert_eq!(out.name, "_Z9addKernelPfS_S_i");
        assert_eq!(out.params.len(), 4);
        // Should have all the instructions plus one label pseudo-instruction.
        // Count: 4 ld.param + 3 mov + 1 mad + 1 setp + 1 bra + 2 cvta + 1 mul
        //      + 1 add.s64 + 1 cvta + 1 add.s64 + 2 ld.global + 1 add.f32
        //      + 1 cvta + 1 add.s64 + 1 st.global + 1 label + 1 ret = 23.
        assert_eq!(out.raw_instructions.len(), 23);
    }
}