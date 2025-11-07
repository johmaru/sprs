#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(i64),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, PartialEq)]
pub struct Item {
    pub ident: String,
    pub blk: Vec<Stmt>,
}

#[derive(Debug, PartialEq)]
pub struct VarDecl {
    pub ident: String,
    pub expr: Expr,
}

#[derive(Debug, PartialEq)]
pub enum Stmt {
    Var(VarDecl),
    If {
        cond: Expr,
        then_blk: Vec<Stmt>,
        else_blk: Option<Vec<Stmt>>,
    },
}
