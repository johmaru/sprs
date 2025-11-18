use crate::type_helper::Type;

#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(i64),               // Value
    Add(Box<Expr>, Box<Expr>), // Lhs, Rhs
    Mul(Box<Expr>, Box<Expr>), // Lhs, Rhs
    Eq(Box<Expr>, Box<Expr>),  // Lhs, Rhs
    If(Box<Expr>, Box<Expr>, Box<Expr>), // Cond, Then, Else

                               // Call(String, Vec<Expr>, Option<Type>), // Ident, Args, RetTy
}

#[derive(Debug, PartialEq)]
pub struct FunctionParam {
    pub ident: String,
}

#[derive(Debug, PartialEq)]
pub enum Item {
    VarItem(VarDecl),
    FunctionItem(Function),
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub ident: String,
    pub params: Vec<FunctionParam>,
    // pub ret_ty: Option<Type>, currently all any
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
