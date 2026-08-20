use crate::front::span::{Span, Spanned};
use crate::front::type_helper::{Type, TypeAnnot};

pub use crate::front::label_name::{LabelName, LabelNamePart, parse_dynamic_label_template};

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(i64),                                                    // Value
    Float(f64),                                                     // Value
    Str(String),                                                    // Value
    Bool(bool),                                                     // Value
    Assign(String, Box<Spanned<Expr>>),                             // name = expr (chained)
    Add(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                    // Lhs, Rhs
    Mul(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                    // Lhs, Rhs
    Minus(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                  // Lhs, Rhs
    Div(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                    // Lhs, Rhs
    Mod(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                    // Lhs, Rhs
    Eq(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                     // Lhs, Rhs
    Neq(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                    // Lhs, Rhs
    Lt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                     // Lhs, Rhs
    Gt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                     // Lhs, Rhs
    Le(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                     // Lhs, Rhs
    Ge(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                     // Lhs, Rhs
    If(Box<Spanned<Expr>>, Box<Spanned<Expr>>, Box<Spanned<Expr>>), // Cond, Then, Else
    Call(String, Vec<Spanned<Expr>>),                               // Ident, Args
    Var(String),                                                    // Ident
    Increment(Box<Spanned<Expr>>),                                  // Ident
    Decrement(Box<Spanned<Expr>>),                                  // Ident
    Neg(Box<Spanned<Expr>>),                                        // Unary minus, e.g. -x
    List(Vec<Spanned<Expr>>),                                       // Elements
    Range(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                  // Start, End
    Index(Box<Spanned<Expr>>, Box<Spanned<Expr>>),                  // Collection, Index
    ModuleAccess(String, String, Vec<Spanned<Expr>>), // Module, functionName, args e.g. module.ident
    FieldAccess(Box<Spanned<Expr>>, String),          // e.g. struct.field
    Unit(),
    Macro(String, Vec<Spanned<Expr>>), // Ident, Args e.g. @lshift(x, 4)
    StructInit(String, Vec<(String, Spanned<Expr>)>), // StructName, Fields
    Atom(LabelName),                   // :ok / :"{x}-item" — immutable atom, no payload
    Label(LabelName, Box<Spanned<Expr>>), // {:name, payload} — payload required
    AttachSlot(String),                // <:name — local operation slot reference (read)
    Try(Box<Spanned<Expr>>),           // Error propagation: expr?
    Match {
        scrutinee: Box<Spanned<Expr>>,
        arms: Vec<ExprMatchArm>,
    }, // Expression match: `match e { case PAT => expr }`
    HeapAlloc(Box<Spanned<Expr>>),     // new(n) — Buffer allocation
    Destroy(Box<Spanned<Expr>>),       // destroy(expr) — explicit Buffer release
    Exist(Box<Spanned<Expr>>),         // exist(expr) — Buffer liveness check

    // System types
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

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionParam {
    pub ident: String,
    pub ty: Option<TypeAnnot>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub enum Item {
    Import(String),
    Package(String),
    VarItem(VarDecl),
    FunctionItem(Function),
    Preprocessor(String),
    FunctionBuildSource { target: String, span: Span },
    FunctionBuildItem(FunctionBuild),
    ClosedLabelSetItem(ClosedLabelSet),
    AtomItem(AtomDef),
    StructItem(Struct),
    HeapAllocItem(HeapAlloc),
}

/// Dedicated FunctionBuild contract directives. These are not runtime macros
/// and must not flow through `Expr::Macro`.
#[derive(Debug, PartialEq, Clone)]
pub enum FunctionBuildDirective {
    Params {
        params: Vec<FunctionParam>,
        span: Span,
    },
    ReturnType {
        ty: Type,
        span: Span,
    },
    Visibility {
        is_public: bool,
        span: Span,
    },
    TypeParam {
        ident: String,
        span: Span,
    },
    When {
        condition: FbCondition,
        ret_ty: Type,
        span: Span,
    },
}

/// Compile-time FunctionBuild `when` condition.
#[derive(Debug, PartialEq, Clone)]
pub enum FbCondition {
    Type(Type),
    Bool(bool),
    Is {
        lhs: Box<FbCondition>,
        rhs: Box<FbCondition>,
    },
    Neq {
        lhs: Box<FbCondition>,
        rhs: Box<FbCondition>,
    },
    And {
        lhs: Box<FbCondition>,
        rhs: Box<FbCondition>,
    },
    Or {
        lhs: Box<FbCondition>,
        rhs: Box<FbCondition>,
    },
    Not {
        inner: Box<FbCondition>,
    },
}

/// Compile-time function contract. Phase 1 is static args / return / visibility only.
#[derive(Debug, PartialEq, Clone)]
pub struct FunctionBuild {
    pub ident: String,
    pub directives: Vec<FunctionBuildDirective>,
    /// Visibility of the FunctionBuild declaration itself (`pub function_build`).
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub ident: String,
    pub params: Vec<FunctionParam>,
    pub blk: Vec<Spanned<Stmt>>,
    pub is_public: bool,
    /// Declared success-path return type (`>> T`). Absent means unannotated.
    /// LLVM ABI still returns `runtime_value_type` so error labels (`{:error, _}`) can propagate.
    pub ret_ty: Option<Type>,
    /// When set, params / ret_ty / is_public are filled from this FunctionBuild.
    pub build_ref: Option<String>,
    pub build_ref_span: Span,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct VarDecl {
    pub ident: String,
    pub expr: Option<Spanned<Expr>>,
    pub ty: Option<TypeAnnot>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct HeapAlloc {
    pub size: Box<Spanned<Expr>>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct AssignStmt {
    pub name: String,
    pub expr: Spanned<Expr>,
    pub span: Span,
}
#[derive(Debug, PartialEq)]
pub struct ClosedLabelSet {
    pub ident: String,
    pub members: Vec<String>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct AtomDef {
    pub ident: String,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub ident: String,
    pub fields: Vec<StructField>,
    pub _methods: Vec<Function>, // currently not implemented
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructField {
    pub ident: String,
    pub ty: Option<Type>,
    pub default_value: Option<Spanned<Expr>>,
    pub span: Span,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Suffix {
    Call(Vec<Spanned<Expr>>),
    Struct(Vec<(String, Spanned<Expr>)>),
}

/// A `case` pattern in a `match` statement.
///
/// v1 supports static names only: `:name` (Atom or Label by name, no payload
/// bind) and `{:name, binder}` (Label only, payload bound to `binder` unless
/// the binder is `"_"`). Dynamic `:"{i}-item"` patterns are rejected.
#[derive(Debug, PartialEq, Clone)]
pub enum MatchPat {
    /// `case :name` — Atom or Label by name (no payload bind)
    Name(LabelName),
    /// `case {:name, binder}` — Label only; binder `"_"` means ignore
    LabelPayload { name: LabelName, binder: String },
    /// `case _` — matches anything; must be the last arm (SEM-017 otherwise)
    Wildcard,
}

/// Body of one `match` arm.
#[derive(Debug, PartialEq)]
pub enum MatchArmBody {
    /// Bind form: `=> expr break;` — value is stored into the `?(var name)` binding.
    ExprBreak(Spanned<Expr>),
    /// No-bind form: `=> { stmts }`
    Block(Vec<Spanned<Stmt>>),
}

/// One `case` arm of a `match` statement.
#[derive(Debug, PartialEq)]
pub struct MatchArm {
    pub pat: MatchPat,
    pub body: MatchArmBody,
    pub span: Span,
}

/// One arm of an expression `match` (`case PAT => expr`, no `break`).
#[derive(Debug, PartialEq, Clone)]
pub struct ExprMatchArm {
    pub pat: MatchPat,
    pub value: Spanned<Expr>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub enum Stmt {
    Var(VarDecl),
    Assign(AssignStmt),
    IndexAssign {
        collection: Spanned<Expr>,
        index: Spanned<Expr>,
        expr: Spanned<Expr>,
        span: Span,
    },
    Expr(Spanned<Expr>),
    If {
        cond: Spanned<Expr>,
        then_blk: Vec<Spanned<Stmt>>,
        else_blk: Option<Vec<Spanned<Stmt>>>,
    },
    While {
        cond: Spanned<Expr>,
        body: Vec<Spanned<Stmt>>,
    },
    Unsafe {
        body: Vec<Spanned<Stmt>>,
        span: Span,
    }, // body runs with `unsafe_depth > 0` (`@raw` / `@free`)
    Defer {
        expr: Spanned<Expr>,
        span: Span,
    }, // queue `expr`; LIFO at scope exit before auto `__drop`
    Return(Option<Spanned<Expr>>),
    Match {
        scrutinee: Spanned<Expr>,
        /// Some(name) when `?(var name)` present
        bind: Option<String>,
        arms: Vec<MatchArm>,
        span: Span,
    },
}
