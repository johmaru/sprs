use logos::Logos;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Plus,
    PlusPlus,
    Star,
    Minus,
    MinusMinus,
    Div,
    Mod,
    Assign,
    EqEq,
    Neq,
    Lt,
    LtColon,
    Gt,
    GtGt,
    Question,
    QuestionLParen,
    Match,
    Case,
    Break,
    FatArrow,
    Le,
    Ge,
    Dot,
    DotDot,
    Semi,
    Comma,
    Colon,
    Macro(String),
    StrLiteral(String),
    Bool(bool),
    If,
    Else,
    While,
    Ident(String),
    Num(i64),
    Float(f64),
    Function,
    Return,
    Preprocessor(String),
    Package,
    Import,
    Var,
    Public,
    Enum,
    Struct,
    Copy,

    Ambi,
    InstanceCreate,
    Destroy,
    Exist,
    Unsafe,
    Defer,

    // System types
    TypeInt,
    TypeFloat,
    TypeBool,
    TypeStr,
    TypeList,
    TypeBuffer,
    TypeRawPtr,
    TypeRange,
    TypeUnit,
    TypeError,
    TypeLabel,
    TypeAtomKw,

    TypeI8,
    TypeU8,
    TypeI16,
    TypeU16,
    TypeI32,
    TypeU32,
    TypeI64,
    TypeU64,

    TypeF16,
    TypeF32,
    TypeF64,
}

#[derive(Logos, Debug, Clone, PartialEq)]
enum RawTok {
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("+")]
    Plus,
    #[token("++")]
    PlusPlus,
    #[token("*")]
    Star,
    #[token("-")]
    Minus,
    #[token("--")]
    MinusMinus,
    #[token("/")]
    Div,
    #[token("%")]
    Mod,
    #[token("=")]
    Assign,
    #[token("==")]
    EqEq,
    #[token("!=")]
    Neq,
    #[token("<:")]
    LtColon,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[regex(r#""(\\.|[^"\\])*""#, |lex| {
        let slice = lex.slice();
        // Strip the surrounding quotes, then unescape the content.
        let raw = &slice[1..slice.len() - 1];
        unescape_sprs_string(raw)
    })]
    StrLiteral(String),
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[regex(r"@[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice()[1..].to_string())]
    MacroIdent(String),
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*!?")]
    Ident,
    #[regex(r"[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?")]
    #[regex(r"[0-9]+[eE][+-]?[0-9]+")]
    Float,
    #[regex(r"[0-9]+")]
    Num,
    #[regex(r"[ \t\r\n\f]+", logos::skip)]
    WS,
    #[regex(r"#[^\n]*", logos::skip, allow_greedy = true)]
    Comment,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("fn")]
    Function,
    #[token(">>")]
    GtGt,
    #[token("?")]
    Question,
    #[token("?(")]
    QuestionLParen,
    #[token("match")]
    Match,
    #[token("case")]
    Case,
    #[token("break")]
    Break,
    #[token("=>")]
    FatArrow,
    #[token("return")]
    Return,
    #[regex(r"#[a-z]+[ \t]+[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().split_ascii_whitespace().nth(1).unwrap().to_owned(), priority = 4)]
    Preprocessor(String),
    #[token("pkg")]
    Package,
    #[token("import")]
    Import,
    #[token("var")]
    Var,
    #[token("pub")]
    Public,
    #[token("enum")]
    Enum,
    #[token("struct")]
    Struct,
    #[token("cp")]
    Copy,
    #[token("ambi")]
    Ambi,
    #[token("new")]
    InstanceCreate,
    #[token("destroy")]
    Destroy,
    #[token("exist")]
    Exist,
    #[token("unsafe")]
    Unsafe,
    #[token("defer")]
    Defer,
    // System types
    #[token("int")]
    TypeInt,
    #[token("fp")]
    TypeFloat,
    #[token("bool")]
    TypeBool,
    #[token("str")]
    TypeStr,
    #[token("list")]
    TypeList,
    #[token("buffer")]
    TypeBuffer,
    #[token("rawptr")]
    TypeRawPtr,
    #[token("range")]
    TypeRange,
    #[token("unit")]
    TypeUnit,
    #[token("err")]
    TypeError,
    #[token("label")]
    TypeLabel,
    #[token("atom")]
    TypeAtomKw,

    #[token("i8")]
    TypeI8,
    #[token("u8")]
    TypeU8,
    #[token("i16")]
    TypeI16,
    #[token("u16")]
    TypeU16,
    #[token("i32")]
    TypeI32,
    #[token("u32")]
    TypeU32,
    #[token("i64")]
    TypeI64,
    #[token("u64")]
    TypeU64,

    #[token("fp16")]
    TypeF16,
    #[token("fp32")]
    TypeF32,
    #[token("fp64")]
    TypeF64,
}

pub struct Lexer<'input> {
    input: &'input str,
    inner: logos::Lexer<'input, RawTok>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            input,
            inner: RawTok::lexer(input),
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<(usize, Token, usize), String>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.inner.next()?;
        let span = self.inner.span();
        let span_start = span.start;
        let span_end = span.end;

        let tok = match res {
            Ok(token_value) => token_value,
            Err(()) => {
                return Some(Err(format!(
                    "invalid token at {}..{}",
                    span_start, span_end
                )));
            }
        };

        let text = &self.input[span_start..span_end];
        let token_value = match tok {
            RawTok::LBrace => Token::LBrace,
            RawTok::RBrace => Token::RBrace,
            RawTok::LBracket => Token::LBracket,
            RawTok::RBracket => Token::RBracket,
            RawTok::LParen => Token::LParen,
            RawTok::RParen => Token::RParen,
            RawTok::Plus => Token::Plus,
            RawTok::PlusPlus => Token::PlusPlus,
            RawTok::Star => Token::Star,
            RawTok::Minus => Token::Minus,
            RawTok::MinusMinus => Token::MinusMinus,
            RawTok::Div => Token::Div,
            RawTok::Mod => Token::Mod,
            RawTok::Assign => Token::Assign,
            RawTok::EqEq => Token::EqEq,
            RawTok::Neq => Token::Neq,
            RawTok::LtColon => Token::LtColon,
            RawTok::Lt => Token::Lt,
            RawTok::Gt => Token::Gt,
            RawTok::GtGt => Token::GtGt,
            RawTok::Question => Token::Question,
            RawTok::QuestionLParen => Token::QuestionLParen,
            RawTok::Match => Token::Match,
            RawTok::Case => Token::Case,
            RawTok::Break => Token::Break,
            RawTok::FatArrow => Token::FatArrow,
            RawTok::Le => Token::Le,
            RawTok::Ge => Token::Ge,
            RawTok::Dot => Token::Dot,
            RawTok::DotDot => Token::DotDot,
            RawTok::Semi => Token::Semi,
            RawTok::Comma => Token::Comma,
            RawTok::Colon => Token::Colon,
            RawTok::StrLiteral(span_start) => Token::StrLiteral(span_start),
            RawTok::If => Token::If,
            RawTok::Else => Token::Else,
            RawTok::While => Token::While,
            RawTok::Ident => Token::Ident(text.to_string()),
            RawTok::MacroIdent(name) => Token::Macro(name),
            RawTok::Num => match text.parse::<i64>() {
                Ok(integer_value) => Token::Num(integer_value),
                Err(span_end) => {
                    return Some(Err(format!(
                        "invalid integer literal '{}': {}",
                        text, span_end
                    )));
                }
            },
            RawTok::Float => match text.parse::<f64>() {
                Ok(float_value) => Token::Float(float_value),
                Err(span_end) => {
                    return Some(Err(format!(
                        "invalid float literal '{}': {}",
                        text, span_end
                    )));
                }
            },
            RawTok::True => Token::Bool(true),
            RawTok::False => Token::Bool(false),
            RawTok::WS => unreachable!(),
            RawTok::Function => Token::Function,
            RawTok::Return => Token::Return,
            RawTok::Preprocessor(value) => Token::Preprocessor(value),
            RawTok::Package => Token::Package,
            RawTok::Import => Token::Import,
            RawTok::Var => Token::Var,
            RawTok::Public => Token::Public,
            RawTok::Enum => Token::Enum,
            RawTok::Struct => Token::Struct,
            RawTok::Comment => return self.next(),
            RawTok::Copy => Token::Copy,
            RawTok::Ambi => Token::Ambi,
            RawTok::InstanceCreate => Token::InstanceCreate,
            RawTok::Destroy => Token::Destroy,
            RawTok::Exist => Token::Exist,
            RawTok::Unsafe => Token::Unsafe,
            RawTok::Defer => Token::Defer,
            // System types
            RawTok::TypeInt => Token::TypeInt,
            RawTok::TypeFloat => Token::TypeFloat,
            RawTok::TypeBool => Token::TypeBool,
            RawTok::TypeStr => Token::TypeStr,
            RawTok::TypeList => Token::TypeList,
            RawTok::TypeBuffer => Token::TypeBuffer,
            RawTok::TypeRawPtr => Token::TypeRawPtr,
            RawTok::TypeRange => Token::TypeRange,
            RawTok::TypeUnit => Token::TypeUnit,
            RawTok::TypeError => Token::TypeError,
            RawTok::TypeLabel => Token::TypeLabel,
            RawTok::TypeAtomKw => Token::TypeAtomKw,

            RawTok::TypeI8 => Token::TypeI8,
            RawTok::TypeU8 => Token::TypeU8,
            RawTok::TypeI16 => Token::TypeI16,
            RawTok::TypeU16 => Token::TypeU16,
            RawTok::TypeI32 => Token::TypeI32,
            RawTok::TypeU32 => Token::TypeU32,
            RawTok::TypeI64 => Token::TypeI64,
            RawTok::TypeU64 => Token::TypeU64,

            RawTok::TypeF16 => Token::TypeF16,
            RawTok::TypeF32 => Token::TypeF32,
            RawTok::TypeF64 => Token::TypeF64,
        };
        Some(Ok((span_start, token_value, span_end)))
    }
}

/// Unescape a Sprs string literal body (the text between the enclosing `"`s).
/// Supports: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`, `\'`, and `\u{XXXX}`.
/// An unknown escape or a dangling backslash is passed through literally so
/// the user sees the raw characters rather than a panic.
fn unescape_sprs_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        // Escape sequence.
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('u') => {
                // Expect `{XXXX}` (1..6 hex digits).
                if chars.next() != Some('{') {
                    // Not a valid \u escape — emit verbatim.
                    out.push_str("\\u");
                    continue;
                }
                let mut hex = String::new();
                let mut depth = 0;
                for ch in chars.by_ref() {
                    depth += 1;
                    if depth > 6 {
                        break;
                    }
                    if ch == '}' {
                        break;
                    }
                    if ch.is_ascii_hexdigit() {
                        hex.push(ch);
                    } else {
                        break;
                    }
                }
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str("\\u{"),
                }
            }
            Some(other) => {
                // Unknown escape: keep the backslash and the character.
                out.push('\\');
                out.push(other);
            }
            None => {
                // Dangling backslash at end of string.
                out.push('\\');
            }
        }
    }
    out
}
