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
    #[token(",")] Comma,
    #[token(";")] Semicolon,
    #[token(":")] Colon,
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("[")] LBracket,
    #[token("]")] RBracket,

    //prefix symbols
    ///prefix for regs
    #[token("%")] Percent,

    ///prefix for predicate guard
    #[token("@")] At,

    #[token("@!")] AtNot,

    ///for opcode modifiers and prefixes
    #[token(".")] Dot,

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

/// tokenizes ptx source string returns list of (token, span pairs)
/// span is byte range of the corresponding token, used for debugging
pub fn tokenize(input: &str) -> Vec<(Token, Span)> {
    Token::lexer(input)
        .spanned()
        .filter_map(|(tok, span)| tok.ok().map(|t| (t, span)))
        .collect()
}
