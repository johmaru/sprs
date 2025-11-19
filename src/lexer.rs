use logos::Logos;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LBrace,
    RBrace,
    LParen,
    RParen,
    Plus,
    Star,
    Eq,
    EqEq,
    Semi,
    Comma,
    If,
    Then,
    Else,
    Ident(String),
    Num(i64),
    Function,
    Return,
}

#[derive(Logos, Debug, Clone, PartialEq)]
enum RawTok {
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("+")]
    Plus,
    #[token("*")]
    Star,
    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"[0-9]+")]
    Num,
    #[regex(r"[ \t\r\n\f]+", logos::skip)]
    WS,
    #[token("fn")]
    Function,
    #[token("return")]
    Return,
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
            RawTok::LParen => Token::LParen,
            RawTok::RParen => Token::RParen,
            RawTok::Plus => Token::Plus,
            RawTok::Star => Token::Star,
            RawTok::Eq => Token::Eq,
            RawTok::EqEq => Token::EqEq,
            RawTok::Semi => Token::Semi,
            RawTok::Comma => Token::Comma,
            RawTok::If => Token::If,
            RawTok::Then => Token::Then,
            RawTok::Else => Token::Else,
            RawTok::Ident => Token::Ident(text.to_string()),
            RawTok::Num => Token::Num(text.parse().unwrap()),
            RawTok::WS => unreachable!(),
            RawTok::Function => Token::Function,
            RawTok::Return => Token::Return,
        };
        Some(Ok((s, t, e)))
    }
}
