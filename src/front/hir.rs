use crate::front::ast::{self, FbCondition, LabelName, MatchPat};
use crate::front::span::{Span, Spanned};
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
    Call {
        callee: CallableRef,
        args: Vec<Expr>,
    },
    Var(String),
    AtomRef(String),
    Increment(Box<Expr>),
    Decrement(Box<Expr>),
    Neg(Box<Expr>),
    List(Vec<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    #[allow(dead_code)]
    ModuleAccess(String, String, Vec<Expr>),
    FieldAccess {
        receiver: Box<Expr>,
        field_name: String,
        struct_ref: StructRef,
        field_index: u32,
    },
    Unit(),
    Macro(String, Vec<Expr>),
    StructInit {
        struct_ref: StructRef,
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

impl Function {
    pub fn contains_unresolved_type(&self) -> bool {
        use crate::front::type_helper::contains_unresolved_type;
        self.params.iter().any(|p| contains_unresolved_type(&p.ty))
            || self.ret_ty.as_ref().is_some_and(contains_unresolved_type)
            || self.body.iter().any(stmt_unresolved)
    }
}

fn stmt_unresolved(stmt: &Stmt) -> bool {
    use crate::front::type_helper::contains_unresolved_type;
    match &stmt.kind {
        StmtKind::Var { binding_ty, init, .. } => {
            contains_unresolved_type(binding_ty) || expr_unresolved(init)
        }
        StmtKind::Assign { rhs, .. } => expr_unresolved(rhs),
        StmtKind::IndexAssign { collection, index, expr } => {
            expr_unresolved(collection) || expr_unresolved(index) || expr_unresolved(expr)
        }
        StmtKind::Expr(expr) | StmtKind::Defer { expr } | StmtKind::Return(Some(expr)) => {
            expr_unresolved(expr)
        }
        StmtKind::Return(None) => false,
        StmtKind::If { cond, then_blk, else_blk } => {
            expr_unresolved(cond)
                || then_blk.iter().any(stmt_unresolved)
                || else_blk.as_ref().is_some_and(|b| b.iter().any(stmt_unresolved))
        }
        StmtKind::While { cond, body } => expr_unresolved(cond) || body.iter().any(stmt_unresolved),
        StmtKind::Unsafe { body } => body.iter().any(stmt_unresolved),
        StmtKind::Match { scrutinee, arms, .. } => {
            expr_unresolved(scrutinee)
                || arms.iter().any(|arm| match &arm.body {
                    MatchArmBody::ExprBreak(expr) => expr_unresolved(expr),
                    MatchArmBody::Block(stmts) => stmts.iter().any(stmt_unresolved),
                })
        }
    }
}

fn expr_unresolved(expr: &Expr) -> bool {
    use crate::front::type_helper::contains_unresolved_type;
    if contains_unresolved_type(&expr.ty) {
        return true;
    }
    match &expr.kind {
        ExprKind::Assign(_, inner)
        | ExprKind::Increment(inner)
        | ExprKind::Decrement(inner)
        | ExprKind::Neg(inner)
        | ExprKind::Try(inner)
        | ExprKind::HeapAlloc(inner)
        | ExprKind::Destroy(inner)
        | ExprKind::Exist(inner)
        | ExprKind::Label(_, inner) => expr_unresolved(inner),
        ExprKind::Add(l, r)
        | ExprKind::Mul(l, r)
        | ExprKind::Minus(l, r)
        | ExprKind::Div(l, r)
        | ExprKind::Mod(l, r)
        | ExprKind::Eq(l, r)
        | ExprKind::Neq(l, r)
        | ExprKind::Lt(l, r)
        | ExprKind::Gt(l, r)
        | ExprKind::Le(l, r)
        | ExprKind::Ge(l, r)
        | ExprKind::Index(l, r)
        | ExprKind::Range(l, r) => expr_unresolved(l) || expr_unresolved(r),
        ExprKind::Call { args, .. } | ExprKind::Macro(_, args) | ExprKind::List(args) => {
            args.iter().any(expr_unresolved)
        }
        ExprKind::ModuleAccess(_, _, args) => args.iter().any(expr_unresolved),
        ExprKind::FieldAccess { receiver, .. } => expr_unresolved(receiver),
        ExprKind::StructInit { fields, struct_ref } => {
            fields.iter().any(|(_, e)| expr_unresolved(e))
                || match struct_ref {
                    StructRef::Generic(id) => id.args.iter().any(contains_unresolved_type),
                    StructRef::Plain(_) => false,
                }
        }
        ExprKind::Match { scrutinee, arms } => {
            expr_unresolved(scrutinee) || arms.iter().any(|arm| expr_unresolved(&arm.value))
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default_value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructId {
    pub module: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructInstanceId {
    pub declaration: StructId,
    pub args: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructRef {
    Plain(String),
    Generic(StructInstanceId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionDeclId {
    pub module: String,
    pub owner: Option<StructId>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionInstanceId {
    pub declaration: FunctionDeclId,
    pub owner_args: Vec<Type>,
    pub function_args: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableRef {
    Plain { module: String, name: String },
    Instance(FunctionInstanceId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTemplate {
    pub id: FunctionDeclId,
    pub params: Vec<ast::FunctionParam>,
    pub ret_ty: Option<Type>,
    pub body: Vec<Spanned<ast::Stmt>>,
    pub owner_params: Vec<String>,
    pub function_params: Vec<String>,
    pub is_public: bool,
    pub when_rules: Vec<(FbCondition, Type)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSpecialization {
    pub id: FunctionInstanceId,
    pub function: Function,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructSpecialization {
    pub id: StructInstanceId,
    pub type_bindings: Vec<(String, Type)>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub id: StructId,
    pub name: String,
    pub type_params: Vec<String>,
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
    pub struct_specializations: Vec<StructSpecialization>,
    pub function_templates: Vec<FunctionTemplate>,
    pub function_specializations: Vec<FunctionSpecialization>,
    pub specialization_requests: Vec<FunctionInstanceId>,
    pub globals: Vec<VarDecl>,
    pub closed_label_sets: Vec<ClosedLabelSet>,
    pub atoms: Vec<AtomDef>,
    pub imports: Vec<String>,
    pub is_main: bool,
}

impl Module {
    pub fn interface(&self) -> ModuleInterface {
        let public_structs: Vec<String> = self
            .structs
            .iter()
            .filter(|s| s.is_public)
            .map(|s| s.name.clone())
            .collect();
        ModuleInterface {
            name: self.name.clone(),
            functions: self.functions.iter().filter(|f| f.is_public).cloned().collect(),
            structs: self.structs.iter().filter(|s| s.is_public).cloned().collect(),
            function_templates: self
                .function_templates
                .iter()
                .filter(|template| {
                    if !template.is_public {
                        return false;
                    }
                    match &template.id.owner {
                        None => true,
                        Some(owner) => public_structs.iter().any(|name| name == &owner.name),
                    }
                })
                .cloned()
                .collect(),
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
    pub function_templates: Vec<FunctionTemplate>,
    pub globals: Vec<VarDecl>,
    pub closed_label_sets: Vec<ClosedLabelSet>,
    pub atoms: Vec<AtomDef>,
}
