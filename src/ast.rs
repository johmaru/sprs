#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(i64),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

#[derive(Debug, PartialEq)]
pub struct Item {
    pub ident: String,
    pub blk : Vec<VarDecl>
}

#[derive(Debug, PartialEq)]
pub struct VarDecl {
    pub ident: String,
    pub expr : Expr,
}