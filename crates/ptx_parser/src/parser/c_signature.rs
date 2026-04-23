//! Parser for C/C++ function signatures.
//!
//! This is a secondary input format to `parse()`. Krishna can pass in a C
//! signature like:
//!
//!   void addKernel(float *A, float *B, float *C, int N)
//!
//! …and get back a `ParsedKernel` with `name` and `params` filled in, but
//! with an empty `instructions` vector. This is useful when Krishna has a
//! kernel's signature (e.g. from demangling a C++ symbol) but doesn't have
//! its PTX body, and just wants the parameter schema for runtime arg
//! interpretation.
//!
//! The grammar we accept is deliberately small:
//!
//!   signature  := return_type ident '(' params? ')'
//!   params     := param (',' param)*
//!   param      := type_name '*'? ident?
//!   type_name  := one of the names in `parse_c_type`
//!
//! No support for: const/volatile qualifiers, templates, namespaces,
//! reference types (&), function pointers, arrays, struct/class params.
//! If Krishna hits any of those, the parser returns an `UnexpectedToken`
//! error and he can preprocess the string on his end before calling us.

use crate::parser::error::ParseError;
use crate::parser::output::{ParamInfo, ParsedKernel, PtxType};

/// Detect whether an input string looks like a PTX file or a C signature.
///
/// Returns `true` for PTX, `false` for C. Heuristic: scan past comments
/// and whitespace; if the first meaningful character is `.` we assume
/// PTX (since every PTX file starts with directives like `.version`).
/// Anything else is treated as a C signature.
pub fn looks_like_ptx(input: &str) -> bool {
    let mut chars = input.char_indices().peekable();
    while let Some(&(_, c)) = chars.peek() {
        // Skip whitespace.
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        // Skip `//` line comment.
        if c == '/' {
            let rest = &input[chars.peek().unwrap().0..];
            if rest.starts_with("//") {
                // consume until newline
                while let Some(&(_, c2)) = chars.peek() {
                    if c2 == '\n' {
                        break;
                    }
                    chars.next();
                }
                continue;
            }
            if rest.starts_with("/*") {
                // consume until `*/`
                chars.next(); // `/`
                chars.next(); // `*`
                while let Some(&(i, _)) = chars.peek() {
                    if input[i..].starts_with("*/") {
                        chars.next();
                        chars.next();
                        break;
                    }
                    chars.next();
                }
                continue;
            }
            return false; // stray `/` — not PTX
        }
        // First real character.
        return c == '.';
    }
    false
}

/// Parse a C function signature into a `ParsedKernel`.
pub fn parse_c_signature(input: &str) -> Result<ParsedKernel, ParseError> {
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
                // Line comment
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                // Block comment
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
                // Identifier.
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                out.push(CTok::Ident(
                    std::str::from_utf8(&bytes[start..i]).unwrap().to_string(),
                ));
            }
            _ => {
                // Unknown char — skip. We'll fail later in parsing with a
                // meaningful error if something was actually required here.
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

    /// signature := <return_type> <name> ( <params>? )
    fn parse_signature(&mut self) -> Result<ParsedKernel, ParseError> {
        // Return type: one or more type-identifiers (e.g. `unsigned int`).
        // We consume and discard — the return type is irrelevant to the
        // caller because kernels typically return void and the runtime
        // doesn't use the return value.
        self.parse_type_words()?;

        // Kernel name.
        let name = self.expect_ident()?;

        // Parameter list.
        self.expect(&CTok::LParen, "`(` before parameter list")?;
        let params = self.parse_params()?;
        self.expect(&CTok::RParen, "`)` after parameter list")?;

        Ok(ParsedKernel {
            name,
            params,
            instructions: Vec::new(),
        })
    }

    /// Consume one or more consecutive identifiers that together form a type.
    /// Accepts `int`, `unsigned int`, `long long`, `uint32_t`, etc. Stops
    /// as soon as the next token isn't an identifier.
    ///
    /// Returns the joined type string (e.g. "unsigned int") so the caller
    /// can map it to a `PtxType`.
    fn parse_type_words(&mut self) -> Result<String, ParseError> {
        self.skip_newlines();
        let mut parts: Vec<String> = Vec::new();
        // Must have at least one identifier.
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
        // Greedily consume additional type-like idents. We stop at
        // anything that would clearly be a param name, which is tricky
        // because `int N` has two idents and the second is the param name.
        //
        // Heuristic: only extend the type if the NEXT ident is a known
        // type modifier like `int`, `long`, `short`, `char`. Otherwise
        // stop and let the caller treat the next ident as a param name.
        while let Some(CTok::Ident(s)) = self.peek() {
            if is_type_modifier(s) {
                parts.push(s.clone());
                self.advance();
            } else {
                break;
            }
        }
        Ok(parts.join(" "))
    }

    fn parse_params(&mut self) -> Result<Vec<ParamInfo>, ParseError> {
        let mut out = Vec::new();
        self.skip_newlines();

        // Empty param list.
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
                // Wasn't `(void)` — it's a real `void` param (uncommon). Rewind.
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

    /// param := <type_words> <star>* <name>?
    fn parse_single_param(&mut self) -> Result<ParamInfo, ParseError> {
        let type_str = self.parse_type_words()?;

        // Zero or more `*` for pointer levels.
        let mut pointer_levels = 0;
        while matches!(self.peek(), Some(CTok::Star)) {
            self.advance();
            pointer_levels += 1;
        }

        // Optional parameter name.
        let name = if let Some(CTok::Ident(s)) = self.peek() {
            let n = s.clone();
            self.advance();
            n
        } else {
            String::new()
        };

        let ptx_type = if pointer_levels > 0 {
            // Any pointer is 8 bytes on 64-bit systems, same as PTX's
            // convention of lowering pointers to `.u64`.
            PtxType::U64
        } else {
            parse_c_type(&type_str).ok_or_else(|| ParseError::UnexpectedToken {
                line: self.line,
                expected: "known C type".to_string(),
                found: type_str,
            })?
        };

        Ok(ParamInfo { name, ptx_type })
    }
}

// ----------------------------------------------------------------------------
// Type mapping
// ----------------------------------------------------------------------------

/// Returns true if a word is part of a compound type (so `parse_type_words`
/// should greedily consume it rather than treating it as a param name).
fn is_type_modifier(s: &str) -> bool {
    matches!(
        s,
        "int"
            | "long"
            | "short"
            | "char"
            | "signed"
            | "unsigned"
            | "const"
            | "volatile"
            | "struct"
    )
}

/// Map a C type string to a `PtxType`. Returns `None` for unknown types.
fn parse_c_type(s: &str) -> Option<PtxType> {
    // Normalize spacing.
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");

    // Strip const/volatile, they don't affect size or representation.
    let cleaned = s
        .replace("const ", "")
        .replace("volatile ", "")
        .trim()
        .to_string();

    Some(match cleaned.as_str() {
        // Floating point
        "float" => PtxType::F32,
        "double" => PtxType::F32, // we lack F64; approximate

        // 32-bit integers
        "int"
        | "signed int"
        | "signed"
        | "int32_t"
        | "i32" => PtxType::S32,
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

        // 8/16-bit integers — widen to 32-bit per C promotion rules.
        "char" | "signed char" | "short" | "short int" | "int8_t" | "int16_t" => PtxType::S32,
        "unsigned char" | "unsigned short" | "unsigned short int" | "uint8_t" | "uint16_t" => {
            PtxType::U32
        }

        // Booleans and predicates
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

    #[test]
    fn detects_ptx_vs_c() {
        assert!(looks_like_ptx(".version 9.1\n"));
        assert!(looks_like_ptx("  .entry foo() {}"));
        assert!(looks_like_ptx("// comment\n.entry foo() {}"));
        assert!(!looks_like_ptx("void foo()"));
        assert!(!looks_like_ptx("int main() { return 0; }"));
    }

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
        assert_eq!(k.params[0].ptx_type, PtxType::U64);
        assert_eq!(k.params[3].name, "N");
        assert_eq!(k.params[3].ptx_type, PtxType::S32);
    }

    #[test]
    fn unsigned_int_compound_type() {
        let k = parse_c_signature("void foo(unsigned int x, int y)").unwrap();
        assert_eq!(k.params[0].name, "x");
        assert_eq!(k.params[0].ptx_type, PtxType::U32);
        assert_eq!(k.params[1].name, "y");
        assert_eq!(k.params[1].ptx_type, PtxType::S32);
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
        // All of these should parse the same way.
        for src in &[
            "void foo(float* p)",
            "void foo(float *p)",
            "void foo(float * p)",
            "void foo(float*p)",
        ] {
            let k = parse_c_signature(src).unwrap();
            assert_eq!(k.params.len(), 1, "failed on: {src}");
            assert_eq!(k.params[0].ptx_type, PtxType::U64);
            assert_eq!(k.params[0].name, "p");
        }
    }

    #[test]
    fn double_pointer() {
        // `float** p` is still 8 bytes at runtime.
        let k = parse_c_signature("void foo(float** p)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::U64);
    }

    #[test]
    fn unnamed_parameter() {
        // Demangled signatures often have no arg names.
        let k = parse_c_signature("void foo(float*, int)").unwrap();
        assert_eq!(k.params.len(), 2);
        assert_eq!(k.params[0].name, "");
        assert_eq!(k.params[0].ptx_type, PtxType::U64);
        assert_eq!(k.params[1].name, "");
        assert_eq!(k.params[1].ptx_type, PtxType::S32);
    }

    #[test]
    fn const_qualifier_ignored() {
        let k = parse_c_signature("void foo(const int a, const float* b)").unwrap();
        assert_eq!(k.params[0].ptx_type, PtxType::S32);
        assert_eq!(k.params[1].ptx_type, PtxType::U64);
    }
}