//! Parser for C/C++ function signatures.
//!
//! This is the second public entry point of the parser crate. It accepts
//! two signature styles:
//!
//!   // Full signature (from source or `__PRETTY_FUNCTION__`):
//!   void addKernel(float *A, float *B, float *C, int N)
//!
//!   // Demangled-style signature (from `cpp_demangle` or c++filt):
//!   addKernel(float*, float*, float*, int)
//!
//! Returns a `ParsedSignature` with `name` and `params` populated. Each
//! param includes both the dereferenced type and the number of pointer
//! levels, so `float*` becomes `{ ptx_type: F32, pointer_levels: 1 }`.
//! Useful when Krishna has a kernel's signature but doesn't have its PTX
//! body, and just wants the parameter schema for interpreting runtime
//! args.
//!
//! Grammar:
//!
//!   signature  := [return_type] ident '(' params? ')'
//!   params     := param (',' param)*
//!   param      := type_name '*'* ident?
//!   type_name  := one of the names handled by `parse_c_type`
//!
//! No support for: templates, namespaces, reference types (&), function
//! pointers, arrays, struct/class params. If Krishna hits any of those,
//! the parser returns `UnexpectedToken` and he can preprocess on his end.

use crate::parser::error::ParseError;
use crate::parser::output::{ParsedSignature, PtxType, SignatureParam};

/// Parse a C function signature into a `ParsedSignature`.
pub fn parse_c_signature(input: &str) -> Result<ParsedSignature, ParseError> {
    let tokens = tokenize_c(input);
    let mut p = CParser {
        tokens,
        cursor: 0,
        line: 1,
    };
    p.parse_signature()
}

// ----------------------------------------------------------------------------
// Tokenization for C signatures
// ----------------------------------------------------------------------------
//
// Kept simple and self-contained. Doesn't reuse the PTX lexer because C's
// lexical rules are different (e.g. `*` is meaningful, `.` is not).

#[derive(Debug, Clone, PartialEq)]
enum CTok {
    Ident(String),
    Star,
    Comma,
    LParen,
    RParen,
    // Newline tracked for error reporting only.
    Newline,
}

fn tokenize_c(input: &str) -> Vec<CTok> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                out.push(CTok::Newline);
                i += 1;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
            }
            b'*' => {
                out.push(CTok::Star);
                i += 1;
            }
            b',' => {
                out.push(CTok::Comma);
                i += 1;
            }
            b'(' => {
                out.push(CTok::LParen);
                i += 1;
            }
            b')' => {
                out.push(CTok::RParen);
                i += 1;
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push(CTok::Ident(
                    std::str::from_utf8(&bytes[start..i]).unwrap().to_string(),
                ));
            }
            _ => {
                // Unknown char; skip silently. Grammar failures are caught
                // downstream with a meaningful error.
                i += 1;
            }
        }
    }
    out
}

// ----------------------------------------------------------------------------
// Parser state and grammar
// ----------------------------------------------------------------------------

struct CParser {
    tokens: Vec<CTok>,
    cursor: usize,
    line: usize,
}

impl CParser {
    fn peek(&self) -> Option<&CTok> {
        self.tokens.get(self.cursor)
    }

    fn advance(&mut self) -> Option<CTok> {
        let t = self.tokens.get(self.cursor).cloned();
        self.cursor += 1;
        if matches!(t, Some(CTok::Newline)) {
            self.line += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(CTok::Newline)) {
            self.advance();
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        self.skip_newlines();
        match self.advance() {
            Some(CTok::Ident(s)) => Ok(s),
            other => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: "identifier".to_string(),
                found: format!("{other:?}"),
            }),
        }
    }

    fn expect(&mut self, want: &CTok, label: &str) -> Result<(), ParseError> {
        self.skip_newlines();
        match self.peek() {
            Some(t) if std::mem::discriminant(t) == std::mem::discriminant(want) => {
                self.advance();
                Ok(())
            }
            other => Err(ParseError::UnexpectedToken {
                line: self.line,
                expected: label.to_string(),
                found: format!("{other:?}"),
            }),
        }
    }

    /// signature := [return_type] name ( params? )
    ///
    /// The return type is optional to support two styles:
    ///   - full signature:    "void foo(int a)"       has return type "void"
    ///   - demangled style:   "foo(int)"              has no return type
    ///
    /// Lookahead: if the first identifier is followed directly by `(`,
    /// the identifier is the function name and there is no return type.
    /// Otherwise, consume a return type then expect the function name.
    fn parse_signature(&mut self) -> Result<ParsedSignature, ParseError> {
        self.skip_newlines();

        let name = if self.starts_with_bare_name() {
            self.expect_ident()?
        } else {
            self.parse_type_words()?;
            self.expect_ident()?
        };

        self.expect(&CTok::LParen, "`(` before parameter list")?;
        let params = self.parse_params()?;
        self.expect(&CTok::RParen, "`)` after parameter list")?;

        Ok(ParsedSignature { name, params })
    }

    /// Lookahead: are we at an identifier that's immediately followed by
    /// `(`? If so the input is a demangled signature and has no return
    /// type. Newlines between the identifier and `(` are tolerated.
    fn starts_with_bare_name(&self) -> bool {
        let mut i = self.cursor;
        while let Some(CTok::Newline) = self.tokens.get(i) {
            i += 1;
        }
        if !matches!(self.tokens.get(i), Some(CTok::Ident(_))) {
            return false;
        }
        i += 1;
        while let Some(CTok::Newline) = self.tokens.get(i) {
            i += 1;
        }
        matches!(self.tokens.get(i), Some(CTok::LParen))
    }

    /// Consume one or more consecutive identifiers that together form a type.
    /// Accepts `int`, `unsigned int`, `long long`, `uint32_t`, etc. Stops
    /// as soon as the next token isn't an identifier that could extend
    /// the type.
    ///
    /// Leading `const` and `volatile` qualifiers are skipped entirely —
    /// they don't affect size or representation.
    fn parse_type_words(&mut self) -> Result<String, ParseError> {
        self.skip_newlines();

        // Skip leading qualifiers.
        while let Some(CTok::Ident(s)) = self.peek() {
            if s == "const" || s == "volatile" {
                self.advance();
            } else {
                break;
            }
        }

        let mut parts: Vec<String> = Vec::new();
        match self.advance() {
            Some(CTok::Ident(s)) => parts.push(s),
            other => {
                return Err(ParseError::UnexpectedToken {
                    line: self.line,
                    expected: "type name".to_string(),
                    found: format!("{other:?}"),
                })
            }
        }

        // Greedily consume additional type-modifier words, and also skip
        // trailing const/volatile.
        loop {
            let next_is_modifier = match self.peek() {
                Some(CTok::Ident(s)) => is_type_modifier(s) || s == "const" || s == "volatile",
                _ => false,
            };
            if !next_is_modifier {
                break;
            }
            // Consume it. If it's a qualifier, don't push into parts.
            if let Some(CTok::Ident(s)) = self.advance() {
                if s != "const" && s != "volatile" {
                    parts.push(s);
                }
            }
        }
        Ok(parts.join(" "))
    }

    fn parse_params(&mut self) -> Result<Vec<SignatureParam>, ParseError> {
        let mut out = Vec::new();
        self.skip_newlines();

        if matches!(self.peek(), Some(CTok::RParen)) {
            return Ok(out);
        }

        // Special case: `(void)` means "no parameters" in C.
        if let Some(CTok::Ident(s)) = self.peek() {
            if s == "void" {
                let save = self.cursor;
                self.advance();
                self.skip_newlines();
                if matches!(self.peek(), Some(CTok::RParen)) {
                    return Ok(out);
                }
                self.cursor = save;
            }
        }

        loop {
            self.skip_newlines();
            let param = self.parse_single_param()?;
            out.push(param);
            self.skip_newlines();
            match self.peek() {
                Some(CTok::Comma) => {
                    self.advance();
                    continue;
                }
                Some(CTok::RParen) | None => break,
                Some(other) => {
                    return Err(ParseError::UnexpectedToken {
                        line: self.line,
                        expected: "`,` or `)`".to_string(),
                        found: format!("{other:?}"),
                    })
                }
            }
        }
        Ok(out)
    }

    /// param := type_words star* ident?
    ///
    /// The returned `SignatureParam` carries the *dereferenced* type — for
    /// `float*`, `ptx_type = F32` and `pointer_levels = 1`. For a plain
    /// `int`, `ptx_type = S32` and `pointer_levels = 0`.
    fn parse_single_param(&mut self) -> Result<SignatureParam, ParseError> {
        let type_str = self.parse_type_words()?;

        let mut pointer_levels: u8 = 0;
        while matches!(self.peek(), Some(CTok::Star)) {
            self.advance();
            pointer_levels = pointer_levels.saturating_add(1);
        }

        let name = if let Some(CTok::Ident(s)) = self.peek() {
            let n = s.clone();
            self.advance();
            n
        } else {
            String::new()
        };

        // Unlike before, ptx_type is the dereferenced type even for pointers.
        // `float*` → ptx_type = F32, pointer_levels = 1.
        let ptx_type = parse_c_type(&type_str).ok_or_else(|| ParseError::UnexpectedToken {
            line: self.line,
            expected: "known C type".to_string(),
            found: type_str,
        })?;

        Ok(SignatureParam {
            name,
            ptx_type,
            pointer_levels,
        })
    }
}

// ----------------------------------------------------------------------------
// Type mapping
// ----------------------------------------------------------------------------

/// Returns true if a word is part of a compound type (so `parse_type_words`
/// should greedily consume it rather than treating it as a param name).
/// Note: `const` and `volatile` are NOT here — they're handled separately
/// in `parse_type_words` so they don't contaminate the returned type string.
fn is_type_modifier(s: &str) -> bool {
    matches!(
        s,
        "int" | "long" | "short" | "char" | "signed" | "unsigned" | "struct"
    )
}

/// Map a C type string to a `PtxType`. Returns `None` for unknown types.
///
/// The input is expected to already have `const` / `volatile` stripped
/// (that happens in `parse_type_words`), so this function only needs to
/// handle pure type spellings.
fn parse_c_type(s: &str) -> Option<PtxType> {
    // Normalize whitespace (turn any run of whitespace into a single space).
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");

    Some(match normalized.as_str() {
        // Floating point
        "float" => PtxType::F32,
        "double" => PtxType::F32, // we lack F64 in PtxType; approximate

        // 32-bit integers
        "int" | "signed int" | "signed" | "int32_t" | "i32" => PtxType::S32,
        "unsigned int" | "unsigned" | "uint32_t" | "u32" => PtxType::U32,

        // 64-bit integers
        "long"
        | "signed long"
        | "long int"
        | "signed long int"
        | "long long"
        | "signed long long"
        | "long long int"
        | "int64_t"
        | "ptrdiff_t" => PtxType::S64,
        "unsigned long"
        | "unsigned long int"
        | "unsigned long long"
        | "unsigned long long int"
        | "uint64_t"
        | "size_t" => PtxType::U64,

        // 8/16-bit integers widen to 32-bit per C promotion rules.
        "char" | "signed char" | "short" | "short int" | "int8_t" | "int16_t" => PtxType::S32,
        "unsigned char" | "unsigned short" | "unsigned short int" | "uint8_t" | "uint16_t" => {
            PtxType::U32
        }

        // Booleans
        "bool" | "_Bool" => PtxType::Pred,

        _ => return None,
    })
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- full signatures -----------------------------------------------

    #[test]
    fn simple_void_kernel() {
        let k = parse_c_signature("void foo()").unwrap();
        assert_eq!(k.name, "foo");
        assert!(k.params.is_empty());
    }

    #[test]
    fn void_param_list() {
        let k = parse_c_signature("void foo(void)").unwrap();
        assert_eq!(k.name, "foo");
        assert!(k.params.is_empty());
    }

    #[test]
    fn pointer_and_int() {
        let k = parse_c_signature("void addKernel(float* A, float* B, float* C, int N)").unwrap();
        assert_eq!(k.name, "addKernel");
        assert_eq!(k.params.len(), 4);
        assert_eq!(k.params[0].name, "A");
        assert_eq!(k.params[0].ptx_type, PtxType::F32);
        assert_eq!(k.params[0].pointer_levels, 1);
        assert_eq!(k.params[3].name, "N");
        assert_eq!(k.params[3].ptx_type, PtxType::S32);
        assert_eq!(k.params[3].pointer_levels, 0);
    }

    #[test]
    fn unsigned_int_compound_type() {
        let k = parse_c_signature("void foo(unsigned int x, int y)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::U32);
        assert_eq!(k.params[0].pointer_levels, 0);
        assert_eq!(k.params[1].ptx_type, PtxType::S32);
        assert_eq!(k.params[1].pointer_levels, 0);
    }

    #[test]
    fn long_types() {
        let k = parse_c_signature("void foo(long a, unsigned long b, long long c)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::S64);
        assert_eq!(k.params[1].ptx_type, PtxType::U64);
        assert_eq!(k.params[2].ptx_type, PtxType::S64);
    }

    #[test]
    fn stdint_types() {
        let k =
            parse_c_signature("void foo(int32_t a, uint32_t b, int64_t c, uint64_t d)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::S32);
        assert_eq!(k.params[1].ptx_type, PtxType::U32);
        assert_eq!(k.params[2].ptx_type, PtxType::S64);
        assert_eq!(k.params[3].ptx_type, PtxType::U64);
    }

    #[test]
    fn star_spacing_variants() {
        for src in &[
            "void foo(float* p)",
            "void foo(float *p)",
            "void foo(float * p)",
            "void foo(float*p)",
        ] {
            let k = parse_c_signature(src).unwrap();
            assert_eq!(k.params.len(), 1, "failed on: {src}");
            assert_eq!(k.params[0].ptx_type, PtxType::F32);
            assert_eq!(k.params[0].pointer_levels, 1);
            assert_eq!(k.params[0].name, "p");
        }
    }

    #[test]
    fn double_pointer() {
        let k = parse_c_signature("void foo(float** p)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::F32);
        assert_eq!(k.params[0].pointer_levels, 2);
    }

    #[test]
    fn const_qualifier_ignored() {
        let k = parse_c_signature("void foo(const int a, const float* b)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::S32);
        assert_eq!(k.params[0].pointer_levels, 0);
        assert_eq!(k.params[1].ptx_type, PtxType::F32);
        assert_eq!(k.params[1].pointer_levels, 1);
    }

    // -- demangled signatures ------------------------------------------

    #[test]
    fn demangled_no_return_no_names() {
        let k = parse_c_signature("addKernel(float*, float*, float*, int)").unwrap();
        assert_eq!(k.name, "addKernel");
        assert_eq!(k.params.len(), 4);
        for i in 0..3 {
            assert_eq!(k.params[i].name, "");
            assert_eq!(k.params[i].ptx_type, PtxType::F32);
            assert_eq!(k.params[i].pointer_levels, 1);
        }
        assert_eq!(k.params[3].name, "");
        assert_eq!(k.params[3].ptx_type, PtxType::S32);
        assert_eq!(k.params[3].pointer_levels, 0);
    }

    #[test]
    fn demangled_empty_params() {
        let k = parse_c_signature("foo()").unwrap();
        assert_eq!(k.name, "foo");
        assert!(k.params.is_empty());
    }

    #[test]
    fn demangled_stdint() {
        let k = parse_c_signature("compute(uint64_t, uint32_t, float*)").unwrap();
        assert_eq!(k.name, "compute");
        assert_eq!(k.params[0].ptx_type, PtxType::U64);
        assert_eq!(k.params[0].pointer_levels, 0);
        assert_eq!(k.params[1].ptx_type, PtxType::U32);
        assert_eq!(k.params[1].pointer_levels, 0);
        assert_eq!(k.params[2].ptx_type, PtxType::F32);
        assert_eq!(k.params[2].pointer_levels, 1);
    }

    #[test]
    fn demangled_compound_types() {
        let k = parse_c_signature("foo(unsigned int, unsigned long long)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::U32);
        assert_eq!(k.params[1].ptx_type, PtxType::U64);
    }

    #[test]
    fn full_sig_also_works_when_signature_unnamed() {
        let k = parse_c_signature("void foo(int, float*)").unwrap();
        assert_eq!(k.name, "foo");
        assert_eq!(k.params[0].ptx_type, PtxType::S32);
        assert_eq!(k.params[0].pointer_levels, 0);
        assert_eq!(k.params[0].name, "");
        assert_eq!(k.params[1].ptx_type, PtxType::F32);
        assert_eq!(k.params[1].pointer_levels, 1);
        assert_eq!(k.params[1].name, "");
    }
}