
use logos::Logos; //used for fast tokenization

///span of token
pub type Span = std::ops::Range<usize>;

//skip whitespace and comments
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\f]+")]  
#[logos(skip r"//[^\n]*")] 
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum Token {
    //punctuation
    #[token(",")]
    Comma,

    #[token(";")]
    Semicolon,

    #[token(":")]
    Colon,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    //prefix symbols
    ///prefix for regs
    #[token("%")]
    Percent,

    ///prefix for predicate guard
    #[token("@")]
    At,

    #[token("@!")]
    AtNot,

    ///for opcode modifiers and prefixes
    #[token(".")]
    Dot,


    ///PTX label starts with $ followed by id chars
    #[regex(r"\$[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Label(String),

    //number literals
    ///float
    #[regex(r"0[fF][0-9A-Fa-f]{8}", |lex| {
        let s = lex.slice();
        u32::from_str_radix(&s[2..], 16).ok()
    })]
    FloatBits(u32),

    ///hex int
    #[regex(r"0[xX][0-9A-Fa-f]+", |lex| {
        let s = lex.slice();
        i64::from_str_radix(&s[2..], 16).ok()
    })]
    IntHex(i64),

    ///decimal int
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntDec(i64),

    //identifiers, starts with letter or underscore, followed by any mix of letters/digits/underscores
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    //newline, just used to track line numbers for debugging
    #[token("\n")]
    Newline,
}

/// Tokenize a PTX source string.
/// tokenizes ptx source string returns list of (token, span pairs)
/// span is byte range of the corresponding token, used for debugging
pub fn tokenize(input: &str) -> Vec<(Token, Span)> {
    Token::lexer(input)
        .spanned()
        .filter_map(|(tok, span)| tok.ok().map(|t| (t, span)))
        .collect()
}

//tests, run with cargo test
#[cfg(test)]
mod tests {
    use super::*;

    fn toks(input: &str) -> Vec<Token> {
        tokenize(input).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn punctuation() {
        assert_eq!(
            toks(",;:(){}[]"),
            vec![
                Token::Comma,
                Token::Semicolon,
                Token::Colon,
                Token::LParen,
                Token::RParen,
                Token::LBrace,
                Token::RBrace,
                Token::LBracket,
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn prefixes() {
        assert_eq!(
            toks("% @ @! ."),
            vec![Token::Percent, Token::At, Token::AtNot, Token::Dot]
        );
    }

    #[test]
    fn identifiers_and_modifiers() {
        // `ld.param.u64` should lex as Ident Dot Ident Dot Ident.
        assert_eq!(
            toks("ld.param.u64"),
            vec![
                Token::Ident("ld".into()),
                Token::Dot,
                Token::Ident("param".into()),
                Token::Dot,
                Token::Ident("u64".into()),
            ]
        );
    }

    #[test]
    fn registers_as_percent_plus_ident() {
        // `%rd1` is two tokens. Parser reassembles.
        assert_eq!(
            toks("%rd1"),
            vec![Token::Percent, Token::Ident("rd1".into())]
        );
    }

    #[test]
    fn special_register_decomposes() {
        // `%tid.x` is Percent Ident("tid") Dot Ident("x"). Parser reassembles.
        assert_eq!(
            toks("%tid.x"),
            vec![
                Token::Percent,
                Token::Ident("tid".into()),
                Token::Dot,
                Token::Ident("x".into()),
            ]
        );
    }

    #[test]
    fn label_includes_dollar() {
        assert_eq!(toks("$L__BB0_2"), vec![Token::Label("$L__BB0_2".into())]);
    }

    #[test]
    fn numeric_literals() {
        assert_eq!(toks("4"), vec![Token::IntDec(4)]);
        assert_eq!(toks("0x42"), vec![Token::IntHex(0x42)]);
        assert_eq!(toks("0f3F800000"), vec![Token::FloatBits(0x3F800000)]);
    }

    #[test]
    fn float_takes_priority_over_hex() {
        // Regression: `0f...` must not be mistaken for a partial `0x...`.
        let t = toks("0f3F800000");
        assert_eq!(t, vec![Token::FloatBits(0x3F800000)]);
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(
            toks("add // this is ignored\n.s32"),
            vec![
                Token::Ident("add".into()),
                Token::Newline,
                Token::Dot,
                Token::Ident("s32".into()),
            ]
        );
    }

    #[test]
    fn block_comment_skipped() {
        assert_eq!(
            toks("add /* block\ncomment */ .s32"),
            vec![
                Token::Ident("add".into()),
                Token::Dot,
                Token::Ident("s32".into()),
            ]
        );
    }

    #[test]
    fn newlines_emitted() {
        assert_eq!(
            toks("a\nb\nc"),
            vec![
                Token::Ident("a".into()),
                Token::Newline,
                Token::Ident("b".into()),
                Token::Newline,
                Token::Ident("c".into()),
            ]
        );
    }

    #[test]
    fn real_ptx_snippet() {
        // One line from the addKernel example.
        let input = "ld.param.u64  %rd1, [_Z9addKernelPfS_S_i_param_0];";
        let t = toks(input);
        assert_eq!(
            t,
            vec![
                Token::Ident("ld".into()),
                Token::Dot,
                Token::Ident("param".into()),
                Token::Dot,
                Token::Ident("u64".into()),
                Token::Percent,
                Token::Ident("rd1".into()),
                Token::Comma,
                Token::LBracket,
                Token::Ident("_Z9addKernelPfS_S_i_param_0".into()),
                Token::RBracket,
                Token::Semicolon,
            ]
        );
    }

    #[test]
    fn predicate_guard() {
        // `@%p1` should be At, Percent, Ident. `@!%p1` should be AtNot, Percent, Ident.
        assert_eq!(
            toks("@%p1"),
            vec![Token::At, Token::Percent, Token::Ident("p1".into())]
        );
        assert_eq!(
            toks("@!%p1"),
            vec![Token::AtNot, Token::Percent, Token::Ident("p1".into())]
        );
    }
}