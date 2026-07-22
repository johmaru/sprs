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
    Gt,
    GtGt,
    Le,
    Ge,
    Dot,
    DotDot,
    Semi,
    Comma,
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
    Preprocessor,
    Package,
    Import,
    Var,
    Public,
    Enum,
    Struct,

    // System types
    TypeInt,
    TypeFloat,
    TypeBool,
    TypeStr,
    TypeUnit,

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
    #[regex(r"`[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice()[1..].to_string())]
    MacroIdent(String),
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*!?")]
    Ident,
    #[regex(r"[0-9]+\.[0-9]+")]
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
    #[token("return")]
    Return,
    #[token("#define")]
    Preprocessor,
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

    // System types
    #[token("int")]
    TypeInt,
    #[token("fp")]
    TypeFloat,
    #[token("bool")]
    TypeBool,
    #[token("str")]
    TypeStr,
    #[token("unit")]
    TypeUnit,

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
        let s = span.start;
        let e = span.end;

        let tok = match res {
            Ok(t) => t,
            Err(()) => return Some(Err(format!("invalid token at {}..{}", s, e))),
        };

        let text = &self.input[s..e];
        let t = match tok {
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
            RawTok::Lt => Token::Lt,
            RawTok::Gt => Token::Gt,
            RawTok::GtGt => Token::GtGt,
            RawTok::Le => Token::Le,
            RawTok::Ge => Token::Ge,
            RawTok::Dot => Token::Dot,
            RawTok::DotDot => Token::DotDot,
            RawTok::Semi => Token::Semi,
            RawTok::Comma => Token::Comma,
            RawTok::StrLiteral(s) => Token::StrLiteral(s),
            RawTok::If => Token::If,
            RawTok::Else => Token::Else,
            RawTok::While => Token::While,
            RawTok::Ident => Token::Ident(text.to_string()),
            RawTok::MacroIdent(name) => Token::Macro(name),
            RawTok::Num => match text.parse::<i64>() {
                Ok(n) => Token::Num(n),
                Err(e) => return Some(Err(format!(
                    "invalid integer literal '{}': {}",
                    text, e
                ))),
            },
            RawTok::Float => match text.parse::<f64>() {
                Ok(f) => Token::Float(f),
                Err(e) => return Some(Err(format!(
                    "invalid float literal '{}': {}",
                    text, e
                ))),
            },
            RawTok::True => Token::Bool(true),
            RawTok::False => Token::Bool(false),
            RawTok::WS => unreachable!(),
            RawTok::Function => Token::Function,
            RawTok::Return => Token::Return,
            RawTok::Preprocessor => Token::Preprocessor,
            RawTok::Package => Token::Package,
            RawTok::Import => Token::Import,
            RawTok::Var => Token::Var,
            RawTok::Public => Token::Public,
            RawTok::Enum => Token::Enum,
            RawTok::Struct => Token::Struct,
            RawTok::Comment => return self.next(),

            // System types
            RawTok::TypeInt => Token::TypeInt,
            RawTok::TypeFloat => Token::TypeFloat,
            RawTok::TypeBool => Token::TypeBool,
            RawTok::TypeStr => Token::TypeStr,
            RawTok::TypeUnit => Token::TypeUnit,

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
        Some(Ok((s, t, e)))
    }
}

/// Unescape a Sprs string literal body (the text between the enclosing `"`s).
/// Supports: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`, `\'`, and `\u{XXXX}`.
/// An unknown escape or a dangling backslash is passed through literally so
/// the user sees the raw characters rather than a panic.
fn unescape_sprs_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
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
