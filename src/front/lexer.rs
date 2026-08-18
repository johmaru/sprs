use crate::front::parse_error::ParserUserError;
use crate::front::span::Span;
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
    QualifiedAtom(String),
    Macro(String),
    StrLiteral(String),
    Bool(bool),
    If,
    Else,
    While,
    Ident(String),
    EscapedIdent(String),
    Num(i64),
    Float(f64),
    Function,
    Use,
    FunctionBuild,
    Private,
    Return,
    Preprocessor(String),
    Package,
    Import,
    Var,
    Public,
    Struct,
    Ambi,
    InstanceCreate,
    Destroy,
    Exist,
    Unsafe,
    Defer,
    Init,
    Source,
    Params,
    ReturnTypeKw,
    Visibility,
    TypeParam,
    When,
    Is,
    NeqKw,
    And,
    Or,
    Not,

    TypeBool,
    TypeStr,
    TypeUnit,
    TypeLabel,

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

impl Token {
    /// Surface spelling of a reserved keyword token, if any.
    pub fn keyword_name(&self) -> Option<&'static str> {
        match self {
            Token::If => Some("if"),
            Token::Else => Some("else"),
            Token::While => Some("while"),
            Token::Function => Some("fn"),
            Token::Use => Some("use"),
            Token::FunctionBuild => Some("function_build"),
            Token::Private => Some("private"),
            Token::Return => Some("return"),
            Token::Package => Some("pkg"),
            Token::Import => Some("import"),
            Token::Var => Some("var"),
            Token::Public => Some("pub"),
            Token::Struct => Some("struct"),
            Token::Ambi => Some("ambi"),
            Token::InstanceCreate => Some("new"),
            Token::Destroy => Some("destroy"),
            Token::Exist => Some("exist"),
            Token::Unsafe => Some("unsafe"),
            Token::Defer => Some("defer"),
            Token::Match => Some("match"),
            Token::Case => Some("case"),
            Token::Break => Some("break"),
            Token::Bool(_) => Some("true/false"),
            Token::Init => Some("init"),
            Token::Source => Some("source"),
            Token::Params => Some("params"),
            Token::ReturnTypeKw => Some("return_type"),
            Token::Visibility => Some("visibility"),
            Token::TypeParam => Some("type_param"),
            Token::When => Some("when"),
            Token::Is => Some("is"),
            Token::NeqKw => Some("neq"),
            Token::And => Some("and"),
            Token::Or => Some("or"),
            Token::Not => Some("not"),
            Token::TypeBool => Some("bool"),
            Token::TypeStr => Some("str"),
            Token::TypeUnit => Some("unit"),
            Token::TypeLabel => Some("label"),
            Token::TypeI8 => Some("i8"),
            Token::TypeU8 => Some("u8"),
            Token::TypeI16 => Some("i16"),
            Token::TypeU16 => Some("u16"),
            Token::TypeI32 => Some("i32"),
            Token::TypeU32 => Some("u32"),
            Token::TypeI64 => Some("i64"),
            Token::TypeU64 => Some("u64"),
            Token::TypeF16 => Some("f16"),
            Token::TypeF32 => Some("f32"),
            Token::TypeF64 => Some("f64"),
            _ => None,
        }
    }
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
    #[regex(r":[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice()[1..].to_string())]
    QualifiedAtom(String),
    #[regex(r#""(\\.|[^"\\])*""#, |lex| {
        let slice = lex.slice();
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
    #[regex(r"@\^[A-Za-z_][A-Za-z0-9_]*")]
    EscapedMacro,
    #[regex(r"@[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice()[1..].to_string())]
    MacroIdent(String),
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"\^[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice()[1..].to_string())]
    EscapedIdent(String),
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
    #[token("use")]
    Use,
    #[token("function_build")]
    FunctionBuild,
    #[token("private")]
    Private,
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
    #[token("struct")]
    Struct,
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
    #[token("init")]
    Init,
    #[token("source")]
    Source,
    #[token("params")]
    Params,
    #[token("return_type")]
    ReturnTypeKw,
    #[token("visibility")]
    Visibility,
    #[token("type_param")]
    TypeParam,
    #[token("when")]
    When,
    #[token("is")]
    Is,
    #[token("neq")]
    NeqKw,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("bool")]
    TypeBool,
    #[token("str")]
    TypeStr,
    #[token("unit")]
    TypeUnit,
    #[token("label")]
    TypeLabel,

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

    #[token("f16")]
    TypeF16,
    #[token("f32")]
    TypeF32,
    #[token("f64")]
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
    type Item = Result<(usize, Token, usize), ParserUserError>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.inner.next()?;
        let span = self.inner.span();
        let span_start = span.start;
        let span_end = span.end;
        let err_span = Span::new(span_start, span_end);

        let tok = match res {
            Ok(token_value) => token_value,
            Err(()) => {
                return Some(Err(ParserUserError::invalid_token(
                    err_span,
                    format!("invalid token at {}..{}", span_start, span_end),
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
            RawTok::QualifiedAtom(name) => Token::QualifiedAtom(name),
            RawTok::StrLiteral(value) => Token::StrLiteral(value),
            RawTok::If => Token::If,
            RawTok::Else => Token::Else,
            RawTok::While => Token::While,
            RawTok::Ident => Token::Ident(text.to_string()),
            RawTok::EscapedIdent(name) => Token::EscapedIdent(name),
            RawTok::EscapedMacro => {
                return Some(Err(ParserUserError::syntax(
                    1,
                    err_span,
                    "escaped identifier is not allowed in macro names",
                    Some("use @name; ^ is only valid on identifiers".to_string()),
                )));
            }
            RawTok::MacroIdent(name) => Token::Macro(name),
            RawTok::Num => match text.parse::<i64>() {
                Ok(integer_value) => Token::Num(integer_value),
                Err(parse_err) => {
                    return Some(Err(ParserUserError::invalid_token(
                        err_span,
                        format!("invalid integer literal '{}': {}", text, parse_err),
                    )));
                }
            },
            RawTok::Float => match text.parse::<f64>() {
                Ok(float_value) => Token::Float(float_value),
                Err(parse_err) => {
                    return Some(Err(ParserUserError::invalid_token(
                        err_span,
                        format!("invalid float literal '{}': {}", text, parse_err),
                    )));
                }
            },
            RawTok::True => Token::Bool(true),
            RawTok::False => Token::Bool(false),
            RawTok::WS => unreachable!(),
            RawTok::Function => Token::Function,
            RawTok::Use => Token::Use,
            RawTok::FunctionBuild => Token::FunctionBuild,
            RawTok::Private => Token::Private,
            RawTok::Return => Token::Return,
            RawTok::Preprocessor(value) => Token::Preprocessor(value),
            RawTok::Package => Token::Package,
            RawTok::Import => Token::Import,
            RawTok::Var => Token::Var,
            RawTok::Public => Token::Public,
            RawTok::Struct => Token::Struct,
            RawTok::Comment => return self.next(),
            RawTok::Ambi => Token::Ambi,
            RawTok::InstanceCreate => Token::InstanceCreate,
            RawTok::Destroy => Token::Destroy,
            RawTok::Exist => Token::Exist,
            RawTok::Unsafe => Token::Unsafe,
            RawTok::Defer => Token::Defer,
            RawTok::Init => Token::Init,
            RawTok::Source => Token::Source,
            RawTok::Params => Token::Params,
            RawTok::ReturnTypeKw => Token::ReturnTypeKw,
            RawTok::Visibility => Token::Visibility,
            RawTok::TypeParam => Token::TypeParam,
            RawTok::When => Token::When,
            RawTok::Is => Token::Is,
            RawTok::NeqKw => Token::NeqKw,
            RawTok::And => Token::And,
            RawTok::Or => Token::Or,
            RawTok::Not => Token::Not,
            RawTok::TypeBool => Token::TypeBool,
            RawTok::TypeStr => Token::TypeStr,
            RawTok::TypeUnit => Token::TypeUnit,
            RawTok::TypeLabel => Token::TypeLabel,
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
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('u') => {
                if chars.next() != Some('{') {
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
                out.push('\\');
                out.push(other);
            }
            None => {
                out.push('\\');
            }
        }
    }
    out
}
