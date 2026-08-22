use crate::front::ast::{FbCondition, LabelName, MatchPat};
use crate::front::span::Span;
use crate::front::type_helper::Type;

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Assign(String, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Minus(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Neq(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Var(String),
    AtomRef(String),
    Increment(Box<Expr>),
    Decrement(Box<Expr>),
    Neg(Box<Expr>),
    List(Vec<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    ModuleAccess(String, String, Vec<Expr>),
    FieldAccess {
        receiver: Box<Expr>,
        field_name: String,
        struct_name: String,
        field_index: u32,
    },
    Unit(),
    Macro(String, Vec<Expr>),
    StructInit {
        struct_name: String,
        fields: Vec<(u32, Expr)>,
    },
    Atom(LabelName),
    Label(LabelName, Box<Expr>),
    AttachSlot(String),
    Try(Box<Expr>),
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<ExprMatchArm>,
    },
    HeapAlloc(Box<Expr>),
    Destroy(Box<Expr>),
    Exist(Box<Expr>),
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

#[derive(Debug, Clone, PartialEq)]
pub struct ExprMatchArm {
    pub pat: MatchPat,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Var {
        name: String,
        binding_ty: Type,
        is_ambi: bool,
        is_annotated: bool,
        init: Expr,
    },
    Assign {
        name: String,
        rhs: Expr,
    },
    IndexAssign {
        collection: Expr,
        index: Expr,
        expr: Expr,
    },
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
    Unsafe {
        body: Vec<Stmt>,
    },
    Defer {
        expr: Expr,
    },
    Return(Option<Expr>),
    Match {
        scrutinee: Expr,
        bind: Option<String>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pat: MatchPat,
    pub body: MatchArmBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchArmBody {
    ExprBreak(Expr),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: String,
    pub ty: Type,
    pub is_ambi: bool,
    pub is_annotated: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub body: Vec<Stmt>,
    pub ret_ty: Option<Type>,
    pub is_public: bool,
    pub type_params: Vec<String>,
    pub when_rules: Vec<(FbCondition, Type)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default_value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<StructField>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: String,
    pub binding_ty: Type,
    pub is_ambi: bool,
    pub is_annotated: bool,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosedLabelSet {
    pub name: String,
    pub members: Vec<String>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomDef {
    pub name: String,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub path: String,
    pub functions: Vec<Function>,
    pub structs: Vec<Struct>,
    pub globals: Vec<VarDecl>,
    pub closed_label_sets: Vec<ClosedLabelSet>,
    pub atoms: Vec<AtomDef>,
    pub imports: Vec<String>,
    pub is_main: bool,
}

impl Module {
    pub fn interface(&self) -> ModuleInterface {
        ModuleInterface {
            name: self.name.clone(),
            functions: self.functions.iter().filter(|f| f.is_public).cloned().collect(),
            structs: self.structs.iter().filter(|s| s.is_public).cloned().collect(),
            globals: Vec::new(),
            closed_label_sets: self.closed_label_sets.iter().filter(|s| s.is_public).cloned().collect(),
            atoms: self.atoms.iter().filter(|a| a.is_public).cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModuleInterface {
    pub name: String,
    pub functions: Vec<Function>,
    pub structs: Vec<Struct>,
    pub globals: Vec<VarDecl>,
    pub closed_label_sets: Vec<ClosedLabelSet>,
    pub atoms: Vec<AtomDef>,
}
