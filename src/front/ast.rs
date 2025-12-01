
use crate::interpreter::type_helper::Type;

#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(i64),                             // Value
    Str(String),                             // Value
    Bool(bool),                              // Value
    Add(Box<Expr>, Box<Expr>),               // Lhs, Rhs
    Mul(Box<Expr>, Box<Expr>),               // Lhs, Rhs
    Minus(Box<Expr>, Box<Expr>),             // Lhs, Rhs
    Div(Box<Expr>, Box<Expr>),               // Lhs, Rhs
    Mod(Box<Expr>, Box<Expr>),               // Lhs, Rhs
    Eq(Box<Expr>, Box<Expr>),                // Lhs, Rhs
    Neq(Box<Expr>, Box<Expr>),               // Lhs, Rhs
    Lt(Box<Expr>, Box<Expr>),                // Lhs, Rhs
    Gt(Box<Expr>, Box<Expr>),                // Lhs, Rhs
    Le(Box<Expr>, Box<Expr>),                // Lhs, Rhs
    Ge(Box<Expr>, Box<Expr>),                // Lhs, Rhs
    If(Box<Expr>, Box<Expr>, Box<Expr>),     // Cond, Then, Else
    Call(String, Vec<Expr>, Option<Type>),   // Ident, Args, RetTy
    Var(String),                             // Ident
    Increment(Box<Expr>),                    // Ident
    Decrement(Box<Expr>),                    // Ident
    List(Vec<Expr>),                         // Elements
    Range(Box<Expr>, Box<Expr>),             // Start, End
    Index(Box<Expr>, Box<Expr>),             // Collection, Index
    ModuleAccess(String, String, Vec<Expr>), // Module, functionName, args e.g. module.ident
}

#[derive(Debug, PartialEq)]
pub struct FunctionParam {
    pub ident: String,
}

#[derive(Debug, PartialEq)]
pub enum Item {
    Import(String),
    Package(String),
    VarItem(VarDecl),
    FunctionItem(Function),
    Preprocessor(String),
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
    Expr(Expr),
    If {
        cond: Expr,
        then_blk: Vec<Stmt>,
        else_blk: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
}
