//! ソースコード位置情報の表現。

/// ソースコード上の範囲をバイトオフセットで表現。
/// lexer.rs の logos::span と互換。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// 空スパン（位置不明・合成ノード用）
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// AST ノードに span を付与するラッパー。
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}
