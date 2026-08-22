use crate::front::ast::{self, FbCondition, Item, MatchPat};
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::function_build::{CallContractError, ResolvedFunctionSignature, resolve_call_contract};
use crate::front::hir;
use crate::front::label_name::{LabelName, LabelNamePart};
use crate::front::span::{Span, Spanned};
use crate::front::type_helper::{
    Type, TypeAnnot, is_error_label_type, join_list_element_types, list_element, list_type,
    reject_payloadless_label_type, resolve_type, types_assignable, types_compatible,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Binding {
    ty: Type,
    is_ambi: bool,
    is_annotated: bool,
}

struct FnSig {
    params: Vec<Option<TypeAnnot>>,
    ret_ty: Option<Type>,
    type_params: Vec<String>,
    when_rules: Vec<(FbCondition, Type)>,
}

struct StructInfo {
    fields: Vec<hir::StructField>,
    field_indices: HashMap<String, u32>,
}

struct Checker<'a> {
    file: String,
    scopes: Vec<HashMap<String, Binding>>,
    fns: HashMap<String, FnSig>,
    structs: HashMap<String, StructInfo>,
    closed_label_sets: HashMap<String, (Vec<String>, bool)>,
    private_closed_label_members: HashSet<String>,
    atom_defs: HashSet<String>,
    private_atom_defs: HashSet<String>,
    attachments: HashSet<String>,
    unsafe_depth: u32,
    current_fn_ret_ty: Option<Type>,
    function_build_contracts: &'a HashMap<String, (Vec<String>, Vec<(FbCondition, Type)>)>,
}

fn semantic(file: &str, span: Span, number: u32, message: String, help: Option<String>) -> SprsError {
    SprsError::Semantic {
        code: ErrorCode {
            category: ErrorCategory::Semantic,
            number,
        },
        location: Location::new(file.to_string(), span),
        message,
        help,
    }
}

fn type_err(
    file: &str,
    span: Span,
    number: u32,
    message: String,
    expected: Option<String>,
    actual: Option<String>,
    help: Option<String>,
) -> SprsError {
    SprsError::Type {
        code: ErrorCode {
            category: ErrorCategory::Type,
            number,
        },
        location: Location::new(file.to_string(), span),
        message,
        expected_type: expected,
        actual_type: actual,
        help,
    }
}

fn is_int_family(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::TypeI8
            | Type::TypeU8
            | Type::TypeI16
            | Type::TypeU16
            | Type::TypeI32
            | Type::TypeU32
            | Type::TypeI64
            | Type::TypeU64
    )
}

fn is_float_family(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Float | Type::TypeF16 | Type::TypeF32 | Type::TypeF64
    )
}

fn infer_binary_arith_type(lhs: &Type, rhs: &Type) -> Type {
    if is_int_family(lhs) && is_int_family(rhs) {
        if lhs == rhs { lhs.clone() } else { Type::Int }
    } else if is_float_family(lhs) && is_float_family(rhs) {
        if lhs == rhs { lhs.clone() } else { Type::Float }
    } else {
        lhs.clone()
    }
}

pub fn check_module(
    items: &[ast::Item],
    module_name: &str,
    path: &str,
    imported_interfaces: &HashMap<String, hir::ModuleInterface>,
    function_build_contracts: &HashMap<String, (Vec<String>, Vec<(FbCondition, Type)>)>,
) -> Result<hir::Module, SprsError> {
    let mut checker = Checker {
        file: path.to_string(),
        scopes: vec![HashMap::new()],
        fns: HashMap::new(),
        structs: HashMap::new(),
        closed_label_sets: HashMap::new(),
        private_closed_label_members: HashSet::new(),
        atom_defs: HashSet::new(),
        private_atom_defs: HashSet::new(),
        attachments: HashSet::new(),
        unsafe_depth: 0,
        current_fn_ret_ty: None,
        function_build_contracts,
    };

    for (_name, iface) in imported_interfaces {
        for s in &iface.structs {
            checker.import_struct(s);
        }
        for set in &iface.closed_label_sets {
            checker.closed_label_sets.insert(
                set.name.clone(),
                (set.members.clone(), set.is_public),
            );
        }
        for atom in &iface.atoms {
            checker.atom_defs.insert(atom.name.clone());
        }
        for f in &iface.functions {
            checker.fns.insert(
                f.name.clone(),
                FnSig {
                    params: f
                        .params
                        .iter()
                        .map(|p| {
                            if p.is_annotated {
                                Some(TypeAnnot {
                                    ty: p.ty.clone(),
                                    ambi: p.is_ambi,
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                    ret_ty: f.ret_ty.clone(),
                    type_params: f.type_params.clone(),
                    when_rules: f.when_rules.clone(),
                },
            );
        }
        for g in &iface.globals {
            if let Some(scope) = checker.scopes.first_mut() {
                scope.insert(
                    g.name.clone(),
                    Binding {
                        ty: g.binding_ty.clone(),
                        is_ambi: g.is_ambi,
                        is_annotated: g.is_annotated,
                    },
                );
            }
        }
    }

    let mut known_structs: HashSet<String> = checker.structs.keys().cloned().collect();
    let mut known_closed: HashSet<String> = checker.closed_label_sets.keys().cloned().collect();
    for item in items {
        match item {
            Item::StructItem(s) => {
                known_structs.insert(s.ident.clone());
            }
            Item::ClosedLabelSetItem(s) => {
                known_closed.insert(s.ident.clone());
            }
            _ => {}
        }
    }

    let mut hir_structs = Vec::new();
    let mut hir_sets = Vec::new();
    let mut hir_atoms = Vec::new();
    let mut hir_globals = Vec::new();
    let mut hir_fns = Vec::new();
    let mut imports = Vec::new();
    let mut is_main = module_name == "main";

    for item in items {
        if let Item::Package(name) = item {
            if name == "main" {
                is_main = true;
            }
        }
        if let Item::Import(name) = item {
            imports.push(name.clone());
        }
    }

    for item in items {
        match item {
            Item::ClosedLabelSetItem(set) => {
                if checker.closed_label_sets.contains_key(&set.ident) {
                    return Err(semantic(
                        path,
                        set.span,
                        4,
                        format!("Duplicate closed label set: {}", set.ident),
                        None,
                    ));
                }
                checker.closed_label_sets.insert(
                    set.ident.clone(),
                    (set.members.clone(), set.is_public),
                );
                hir_sets.push(hir::ClosedLabelSet {
                    name: set.ident.clone(),
                    members: set.members.clone(),
                    is_public: set.is_public,
                    span: set.span,
                });
            }
            Item::AtomItem(def) => {
                if checker.atom_defs.contains(&def.ident) {
                    return Err(semantic(
                        path,
                        def.span,
                        4,
                        format!("Duplicate label: {}", def.ident),
                        None,
                    ));
                }
                checker.atom_defs.insert(def.ident.clone());
                hir_atoms.push(hir::AtomDef {
                    name: def.ident.clone(),
                    is_public: def.is_public,
                    span: def.span,
                });
            }
            Item::StructItem(s) => {
                let mut fields = Vec::new();
                let mut field_indices = HashMap::new();
                for (idx, field) in s.fields.iter().enumerate() {
                    let mut ty = field.ty.clone().unwrap_or(Type::Any);
                    resolve_type(&mut ty, &known_structs, &known_closed, Some(&s.ident)).map_err(|message| {
                        semantic(path, field.span, 11, message, None)
                    })?;
                    reject_payloadless_label_type(&ty).map_err(|message| {
                        semantic(path, field.span, 11, message, None)
                    })?;
                    let default_value = match &field.default_value {
                        Some(expr) => Some(checker.check_expr(expr, Some(&ty))?),
                        None => None,
                    };
                    field_indices.insert(field.ident.clone(), idx as u32);
                    fields.push(hir::StructField {
                        name: field.ident.clone(),
                        ty,
                        default_value,
                        span: field.span,
                    });
                }
                let hs = hir::Struct {
                    name: s.ident.clone(),
                    fields: fields.clone(),
                    is_public: s.is_public,
                    span: s.span,
                };
                checker.structs.insert(
                    s.ident.clone(),
                    StructInfo {
                        fields,
                        field_indices,
                    },
                );
                hir_structs.push(hs);
            }
            _ => {}
        }
    }

    for item in items {
        if let Item::FunctionItem(func) = item {
            let mut params = Vec::new();
            for p in &func.params {
                let mut annot = p.ty.clone();
                if let Some(a) = &mut annot {
                    resolve_type(&mut a.ty, &known_structs, &known_closed, None).map_err(|message| {
                        semantic(path, p.span, 11, message, None)
                    })?;
                    reject_payloadless_label_type(&a.ty).map_err(|message| {
                        semantic(path, p.span, 11, message, None)
                    })?;
                }
                params.push(annot);
            }
            let mut ret_ty = func.ret_ty.clone();
            if let Some(ty) = &mut ret_ty {
                resolve_type(ty, &known_structs, &known_closed, None).map_err(|message| {
                    semantic(path, func.span, 11, message, None)
                })?;
                reject_payloadless_label_type(ty).map_err(|message| {
                    semantic(path, func.span, 11, message, None)
                })?;
            }
            let (type_params, when_rules) = match &func.build_ref {
                Some(name) => function_build_contracts
                    .get(name)
                    .cloned()
                    .unwrap_or_default(),
                None => (Vec::new(), Vec::new()),
            };
            let llvm_name = if func.ident == "main" {
                crate::naming::INTERNAL_MAIN_FN.to_string()
            } else {
                func.ident.clone()
            };
            checker.fns.insert(
                llvm_name.clone(),
                FnSig {
                    params: params.clone(),
                    ret_ty: ret_ty.clone(),
                    type_params,
                    when_rules,
                },
            );
            if func.ident != llvm_name {
                checker.fns.insert(
                    func.ident.clone(),
                    FnSig {
                        params,
                        ret_ty,
                        type_params: function_build_contracts
                            .get(func.build_ref.as_deref().unwrap_or(""))
                            .map(|c| c.0.clone())
                            .unwrap_or_default(),
                        when_rules: function_build_contracts
                            .get(func.build_ref.as_deref().unwrap_or(""))
                            .map(|c| c.1.clone())
                            .unwrap_or_default(),
                    },
                );
            }
        }
        if let Item::VarItem(var) = item {
            let mut annot = var.ty.clone();
            if let Some(a) = &mut annot {
                resolve_type(&mut a.ty, &known_structs, &known_closed, None).map_err(|message| {
                    semantic(path, var.span, 11, message, None)
                })?;
                reject_payloadless_label_type(&a.ty).map_err(|message| {
                    semantic(path, var.span, 11, message, None)
                })?;
            }
            let expected = annot.as_ref().map(|a| &a.ty);
            let init = match &var.expr {
                Some(expr) => Some(checker.check_expr_in(expr, expected)?),
                None => None,
            };
            if let (Some(expected), Some(init)) = (expected, init.as_ref()) {
                if !types_assignable(expected, &init.ty) {
                    return Err(checker.type_mismatch_assign(
                        var.span,
                        format!(
                            "Type mismatch: cannot assign {} to fixed binding `{}` of type {}",
                            init.ty, var.ident, expected
                        ),
                        expected,
                        &init.ty,
                    ));
                }
                if let Some(expr) = &var.expr {
                    checker.check_list_literal_elements(expr, expected)?;
                }
            }
            let binding_ty = match &annot {
                Some(a) => a.ty.clone(),
                None => init.as_ref().map(|e| e.ty.clone()).unwrap_or(Type::Unit),
            };
            let (is_ambi, is_annotated) = match &annot {
                Some(a) => (a.ambi, true),
                None => (false, false),
            };
            checker.bind(
                var.ident.clone(),
                Binding {
                    ty: binding_ty.clone(),
                    is_ambi,
                    is_annotated,
                },
            );
            hir_globals.push(hir::VarDecl {
                name: var.ident.clone(),
                binding_ty,
                is_ambi,
                is_annotated,
                init,
                span: var.span,
            });
        }
    }

    for item in items {
        if let Item::FunctionItem(func) = item {
            hir_fns.push(checker.check_function(func, &known_structs, &known_closed)?);
        }
    }

    Ok(hir::Module {
        name: module_name.to_string(),
        path: path.to_string(),
        functions: hir_fns,
        structs: hir_structs,
        globals: hir_globals,
        closed_label_sets: hir_sets,
        atoms: hir_atoms,
        imports,
        is_main,
    })
}

impl Checker<'_> {
    fn import_struct(&mut self, s: &hir::Struct) {
        let mut field_indices = HashMap::new();
        for (idx, f) in s.fields.iter().enumerate() {
            field_indices.insert(f.name.clone(), idx as u32);
        }
        self.structs.insert(
            s.name.clone(),
            StructInfo {
                fields: s.fields.clone(),
                field_indices,
            },
        );
    }

    fn location_empty(span: Span) -> Location {
        Location::new(String::new(), span)
    }

    fn bind(&mut self, name: String, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, binding);
        }
    }

    fn get_binding(&self, name: &str) -> Option<Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(b.clone());
            }
        }
        None
    }

    fn set_binding_type(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(b) = scope.get_mut(name) {
                b.ty = ty;
                return;
            }
        }
    }

    fn is_visible_atom_def(&self, name: &str) -> bool {
        self.atom_defs.contains(name) && !self.private_atom_defs.contains(name)
    }

    fn resolve_closed_label_member(&self, name: &str, span: Span) -> Result<Option<String>, SprsError> {
        let Some((set, member)) = name.split_once('.') else {
            return Ok(None);
        };
        let undefined = || {
            semantic(
                &self.file,
                span,
                4,
                format!("Undefined closed label member: {}", name),
                None,
            )
        };
        let Some((members, _)) = self.closed_label_sets.get(set) else {
            return Err(undefined());
        };
        let member_known = members.iter().any(|known| known == member);
        if self.private_closed_label_members.contains(name) || !member_known {
            return Err(undefined());
        }
        Ok(Some(set.to_string()))
    }

    fn type_mismatch_assign(&self, span: Span, message: String, expected: &Type, actual: &Type) -> SprsError {
        type_err(
            &self.file,
            span,
            6,
            message,
            Some(format!("{expected}")),
            Some(format!("{actual}")),
            None,
        )
    }

    fn check_list_literal_elements(&self, expr: &Spanned<ast::Expr>, expected: &Type) -> Result<(), SprsError> {
        let ast::Expr::List(elements) = &expr.node else {
            return Ok(());
        };
        let Some(elem_ty) = list_element(expected) else {
            return Ok(());
        };
        if matches!(elem_ty, Type::Any) {
            return Ok(());
        }
        for element in elements {
            let actual = self.infer_type(element);
            if !types_assignable(elem_ty, &actual) {
                return Err(self.type_mismatch_assign(
                    element.span,
                    format!("Type mismatch: list element has {actual}, expected {elem_ty}"),
                    elem_ty,
                    &actual,
                ));
            }
        }
        Ok(())
    }

    fn infer_type(&self, expr: &Spanned<ast::Expr>) -> Type {
        self.infer_type_in(expr, None)
    }

    fn infer_type_in(&self, expr: &Spanned<ast::Expr>, expected: Option<&Type>) -> Type {
        if let ast::Expr::List(_) = &expr.node {
            if let Some(exp_elem) = expected.and_then(list_element) {
                return list_type(exp_elem.clone());
            }
        }
        match &expr.node {
            ast::Expr::Number(_) => Type::Int,
            ast::Expr::Float(_) => Type::Float,
            ast::Expr::Str(_) => Type::Str,
            ast::Expr::Bool(_) => Type::Bool,
            ast::Expr::Unit() => Type::Unit,
            ast::Expr::Var(name) => {
                if let Some(binding) = self.get_binding(name) {
                    binding.ty
                } else if self.is_visible_atom_def(name) {
                    Type::App("Atom".into(), vec![Type::Atom(name.clone())])
                } else {
                    Type::Any
                }
            }
            ast::Expr::TypeI8 => Type::TypeI8,
            ast::Expr::TypeU8 => Type::TypeU8,
            ast::Expr::TypeI16 => Type::TypeI16,
            ast::Expr::TypeU16 => Type::TypeU16,
            ast::Expr::TypeI32 => Type::TypeI32,
            ast::Expr::TypeU32 => Type::TypeU32,
            ast::Expr::TypeI64 => Type::TypeI64,
            ast::Expr::TypeU64 => Type::TypeU64,
            ast::Expr::TypeF16 => Type::TypeF16,
            ast::Expr::TypeF32 => Type::TypeF32,
            ast::Expr::TypeF64 => Type::TypeF64,
            ast::Expr::Eq(_, _)
            | ast::Expr::Neq(_, _)
            | ast::Expr::Lt(_, _)
            | ast::Expr::Gt(_, _)
            | ast::Expr::Le(_, _)
            | ast::Expr::Ge(_, _) => Type::Bool,
            ast::Expr::Add(lhs, rhs)
            | ast::Expr::Mul(lhs, rhs)
            | ast::Expr::Minus(lhs, rhs)
            | ast::Expr::Div(lhs, rhs)
            | ast::Expr::Mod(lhs, rhs) => {
                infer_binary_arith_type(&self.infer_type(lhs), &self.infer_type(rhs))
            }
            ast::Expr::Assign(_, rhs) => self.infer_type(rhs),
            ast::Expr::Increment(value) | ast::Expr::Decrement(value) | ast::Expr::Neg(value) => {
                self.infer_type(value)
            }
            ast::Expr::Match { arms, .. } => {
                let mut result: Option<Type> = None;
                for arm in arms {
                    let arm_ty = self.infer_type(&arm.value);
                    result = Some(match result {
                        None => arm_ty,
                        Some(t) if types_compatible(&t, &arm_ty) => {
                            if t != Type::Any { t } else { arm_ty }
                        }
                        Some(_) => Type::Any,
                    });
                    if result == Some(Type::Any) {
                        break;
                    }
                }
                result.unwrap_or(Type::Any)
            }
            ast::Expr::Call(name, args) => self.infer_call_return_type(name, args),
            ast::Expr::ModuleAccess(_, function_name, args) => {
                self.infer_call_return_type(function_name, args)
            }
            ast::Expr::Macro(ident, args) => self.macro_result_type(ident, args),
            ast::Expr::List(elements) => {
                let elem_tys: Vec<Type> = elements.iter().map(|e| self.infer_type(e)).collect();
                list_type(join_list_element_types(&elem_tys))
            }
            ast::Expr::Range(_, _) => Type::Range,
            ast::Expr::Try(inner) => self.infer_type(inner),
            ast::Expr::StructInit(name, _) => Type::Struct(name.clone()),
            ast::Expr::HeapAlloc(_) => Type::Buffer,
            ast::Expr::Destroy(_) => Type::Unit,
            ast::Expr::Exist(_) => Type::Bool,
            ast::Expr::Atom(name) => match name {
                LabelName::Static(static_name) => {
                    match self.resolve_closed_label_member(static_name, expr.span) {
                        Ok(Some(set)) => Type::ClosedLabelSet(set),
                        _ => Type::App("Atom".into(), vec![Type::Atom(static_name.clone())]),
                    }
                }
                LabelName::Dynamic(_) => Type::AtomVal,
            },
            ast::Expr::Label(name, payload) => {
                let payload_ty = self.infer_type(payload);
                match name {
                    LabelName::Static(static_name) => {
                        if matches!(payload_ty, Type::Unit) {
                            Type::App("Label".into(), vec![Type::Atom(static_name.clone())])
                        } else {
                            Type::App("Label".into(), vec![Type::Atom(static_name.clone()), payload_ty])
                        }
                    }
                    LabelName::Dynamic(_) => {
                        if matches!(payload_ty, Type::Unit) {
                            Type::Label
                        } else {
                            Type::App("Label".into(), vec![payload_ty])
                        }
                    }
                }
            }
            ast::Expr::AttachSlot(_) => Type::Any,
            ast::Expr::FieldAccess(lhs, rhs) => {
                if let Type::Struct(struct_name) = self.infer_type(lhs) {
                    if let Some(def) = self.structs.get(&struct_name) {
                        if let Some(field) = def.fields.iter().find(|f| f.name == *rhs) {
                            return field.ty.clone();
                        }
                    }
                }
                Type::Any
            }
            ast::Expr::Index(collection, _) => list_element(&self.infer_type(collection))
                .cloned()
                .unwrap_or(Type::Any),
        }
    }

    fn infer_call_return_type(&self, name: &str, args: &[Spanned<ast::Expr>]) -> Type {
        let Some(sig) = self.fns.get(name) else {
            return Type::Any;
        };
        if sig.type_params.is_empty() && sig.when_rules.is_empty() {
            return sig.ret_ty.clone().unwrap_or(Type::Any);
        }
        let actuals: Vec<Type> = args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let expected = sig.params.get(idx).and_then(|p| p.as_ref()).map(|a| &a.ty);
                self.infer_type_in(arg, expected)
            })
            .collect();
        let contract = self.contract_from_sig(sig);
        resolve_call_contract(&contract, &actuals)
            .ok()
            .flatten()
            .unwrap_or(Type::Any)
    }

    fn contract_from_sig(&self, sig: &FnSig) -> ResolvedFunctionSignature {
        ResolvedFunctionSignature {
            params: sig
                .params
                .iter()
                .map(|ty| ast::FunctionParam {
                    ident: String::new(),
                    ty: ty.clone(),
                    span: Span::DUMMY,
                })
                .collect(),
            ret_ty: sig.ret_ty.clone(),
            is_public: false,
            type_params: sig.type_params.clone(),
            when_rules: sig.when_rules.clone(),
        }
    }

    fn macro_result_type(&self, ident: &str, args: &[Spanned<ast::Expr>]) -> Type {
        match ident {
            "cast" => {
                if args.len() >= 2 {
                    self.infer_type(&args[1])
                } else {
                    Type::Any
                }
            }
            "fcast" => Type::Str,
            "lshift" | "rshift" => {
                if !args.is_empty() {
                    self.infer_type(&args[0])
                } else {
                    Type::Any
                }
            }
            "not" => Type::Bool,
            "raw" => Type::RawPtr,
            "free" => Type::Unit,
            "error" => {
                if args.is_empty() {
                    Type::App("Label".into(), vec![Type::Atom("error".into())])
                } else {
                    Type::App("Label".into(), vec![Type::Atom("error".into()), self.infer_type(&args[0])])
                }
            }
            "label_is" => Type::Bool,
            "label_name" => Type::Str,
            "label_payload" => Type::Any,
            "init" => Type::Any,
            "clone" | "move" => {
                if !args.is_empty() {
                    self.infer_type(&args[0])
                } else {
                    Type::Any
                }
            }
            "list_push" => Type::Unit,
            _ => Type::Any,
        }
    }

    fn check_call_arguments(&self, fn_name: &str, args: &[Spanned<ast::Expr>]) -> Result<(), SprsError> {
        let Some(sig) = self.fns.get(fn_name) else {
            return Ok(());
        };
        let actuals: Vec<Type> = args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let expected = sig.params.get(idx).and_then(|p| p.as_ref()).map(|a| &a.ty);
                self.infer_type_in(arg, expected)
            })
            .collect();
        for (idx, arg) in args.iter().enumerate() {
            if let Some(expected) = sig.params.get(idx).and_then(|p| p.as_ref()).map(|a| &a.ty) {
                self.check_list_literal_elements(arg, expected)?;
            }
        }
        let contract = self.contract_from_sig(sig);
        match resolve_call_contract(&contract, &actuals) {
            Ok(_) => Ok(()),
            Err(CallContractError::Arity { expected, actual }) => {
                let span = args.first().map(|a| a.span).unwrap_or(Span::DUMMY);
                Err(semantic(
                    &self.file,
                    span,
                    16,
                    format!(
                        "Argument count mismatch: function `{}` expects {} argument(s), found {}",
                        fn_name, expected, actual
                    ),
                    None,
                ))
            }
            Err(err) => {
                let span = args.first().map(|a| a.span).unwrap_or(Span::DUMMY);
                let message = match &err {
                    CallContractError::TypeConflict { message } => {
                        format!("Type mismatch in call to `{}`: {}", fn_name, message)
                    }
                    CallContractError::UnresolvedTypeParam { name } => {
                        format!(
                            "Type mismatch in call to `{}`: type parameter `{}` was not resolved to a concrete type",
                            fn_name, name
                        )
                    }
                    CallContractError::NotConcrete { message } => {
                        format!("Type mismatch in call to `{}`: {}", fn_name, message)
                    }
                    CallContractError::MultipleMatches => {
                        format!(
                            "Type mismatch in call to `{}`: multiple `when` rules matched",
                            fn_name
                        )
                    }
                    CallContractError::Arity { .. } => unreachable!("handled above"),
                };
                Err(type_err(&self.file, span, 7, message, None, None, None))
            }
        }
    }

    fn check_dynamic_label_parts(&self, parts: &[LabelNamePart]) -> Result<(), SprsError> {
        for part in parts {
            if let LabelNamePart::Ident(ident) = part {
                let binding = self.get_binding(ident).ok_or_else(|| SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 2,
                    },
                    location: Self::location_empty(Span::DUMMY),
                    message: format!("Undefined variable in dynamic label name: {}", ident),
                    help: None,
                })?;
                match &binding.ty {
                    Type::Int | Type::Bool | Type::Str | Type::Any | Type::TypeI64 => {}
                    other => {
                        return Err(SprsError::Semantic {
                            code: ErrorCode {
                                category: ErrorCategory::Semantic,
                                number: 3,
                            },
                            location: Self::location_empty(Span::DUMMY),
                            message: format!(
                                "dynamic label name part `{}` has type {}; only int/bool/str allowed",
                                ident, other
                            ),
                            help: None,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn check_label_name(&self, name: &LabelName, span: Span) -> Result<(), SprsError> {
        match name {
            LabelName::Static(static_name) => {
                self.resolve_closed_label_member(static_name, span)?;
                Ok(())
            }
            LabelName::Dynamic(parts) => self.check_dynamic_label_parts(parts),
        }
    }

    fn check_expr(&mut self, expr: &Spanned<ast::Expr>, expected: Option<&Type>) -> Result<hir::Expr, SprsError> {
        self.check_expr_in(expr, expected)
    }

    fn check_expr_in(
        &mut self,
        expr: &Spanned<ast::Expr>,
        expected: Option<&Type>,
    ) -> Result<hir::Expr, SprsError> {
        let span = expr.span;
        let kind_ty = match &expr.node {
            ast::Expr::Number(n) => (hir::ExprKind::Number(*n), Type::Int),
            ast::Expr::Float(f) => (hir::ExprKind::Float(*f), Type::Float),
            ast::Expr::Str(s) => (hir::ExprKind::Str(s.clone()), Type::Str),
            ast::Expr::Bool(b) => (hir::ExprKind::Bool(*b), Type::Bool),
            ast::Expr::Unit() => (hir::ExprKind::Unit(), Type::Unit),
            ast::Expr::TypeI8 => (hir::ExprKind::TypeI8, Type::TypeI8),
            ast::Expr::TypeU8 => (hir::ExprKind::TypeU8, Type::TypeU8),
            ast::Expr::TypeI16 => (hir::ExprKind::TypeI16, Type::TypeI16),
            ast::Expr::TypeU16 => (hir::ExprKind::TypeU16, Type::TypeU16),
            ast::Expr::TypeI32 => (hir::ExprKind::TypeI32, Type::TypeI32),
            ast::Expr::TypeU32 => (hir::ExprKind::TypeU32, Type::TypeU32),
            ast::Expr::TypeI64 => (hir::ExprKind::TypeI64, Type::TypeI64),
            ast::Expr::TypeU64 => (hir::ExprKind::TypeU64, Type::TypeU64),
            ast::Expr::TypeF16 => (hir::ExprKind::TypeF16, Type::TypeF16),
            ast::Expr::TypeF32 => (hir::ExprKind::TypeF32, Type::TypeF32),
            ast::Expr::TypeF64 => (hir::ExprKind::TypeF64, Type::TypeF64),
            ast::Expr::Var(name) => {
                if self.get_binding(name).is_some() {
                    (hir::ExprKind::Var(name.clone()), self.infer_type(expr))
                } else if self.is_visible_atom_def(name) {
                    (
                        hir::ExprKind::AtomRef(name.clone()),
                        Type::App("Atom".into(), vec![Type::Atom(name.clone())]),
                    )
                } else {
                    return Err(semantic(
                        &self.file,
                        span,
                        2,
                        format!("Undefined variable: {}", name),
                        None,
                    ));
                }
            }
            ast::Expr::Assign(name, rhs) => {
                let rhs_h = self.check_named_assign(name, rhs, span)?;
                let ty = rhs_h.ty.clone();
                (hir::ExprKind::Assign(name.clone(), Box::new(rhs_h)), ty)
            }
            ast::Expr::Add(l, r) => self.bin(l, r, hir::ExprKind::Add)?,
            ast::Expr::Mul(l, r) => self.bin(l, r, hir::ExprKind::Mul)?,
            ast::Expr::Minus(l, r) => self.bin(l, r, hir::ExprKind::Minus)?,
            ast::Expr::Div(l, r) => self.bin(l, r, hir::ExprKind::Div)?,
            ast::Expr::Mod(l, r) => self.bin(l, r, hir::ExprKind::Mod)?,
            ast::Expr::Eq(l, r) => self.bin_bool(l, r, hir::ExprKind::Eq)?,
            ast::Expr::Neq(l, r) => self.bin_bool(l, r, hir::ExprKind::Neq)?,
            ast::Expr::Lt(l, r) => self.bin_bool(l, r, hir::ExprKind::Lt)?,
            ast::Expr::Gt(l, r) => self.bin_bool(l, r, hir::ExprKind::Gt)?,
            ast::Expr::Le(l, r) => self.bin_bool(l, r, hir::ExprKind::Le)?,
            ast::Expr::Ge(l, r) => self.bin_bool(l, r, hir::ExprKind::Ge)?,
            ast::Expr::Increment(inner) => {
                let h = self.check_expr(inner, None)?;
                let ty = h.ty.clone();
                (hir::ExprKind::Increment(Box::new(h)), ty)
            }
            ast::Expr::Decrement(inner) => {
                let h = self.check_expr(inner, None)?;
                let ty = h.ty.clone();
                (hir::ExprKind::Decrement(Box::new(h)), ty)
            }
            ast::Expr::Neg(inner) => {
                let h = self.check_expr(inner, None)?;
                let ty = h.ty.clone();
                (hir::ExprKind::Neg(Box::new(h)), ty)
            }
            ast::Expr::Call(name, args) => {
                self.check_call_arguments(name, args)?;
                let args_h = self.check_args(args)?;
                (hir::ExprKind::Call(name.clone(), args_h), self.infer_call_return_type(name, args))
            }
            ast::Expr::ModuleAccess(module_name, function_name, args) => {
                self.check_call_arguments(function_name, args)?;
                let args_h = self.check_args(args)?;
                (
                    hir::ExprKind::ModuleAccess(module_name.clone(), function_name.clone(), args_h),
                    self.infer_call_return_type(function_name, args),
                )
            }
            ast::Expr::Macro(ident, args) => self.check_macro(ident, args, span)?,
            ast::Expr::List(elements) => {
                if let Some(expected) = expected {
                    self.check_list_literal_elements(expr, expected)?;
                }
                let elems: Result<Vec<_>, _> = elements
                    .iter()
                    .map(|e| {
                        let exp = expected.and_then(list_element);
                        self.check_expr(e, exp)
                    })
                    .collect();
                let elems = elems?;
                let ty = self.infer_type_in(expr, expected);
                (hir::ExprKind::List(elems), ty)
            }
            ast::Expr::Range(s, e) => {
                let s = self.check_expr(s, None)?;
                let e = self.check_expr(e, None)?;
                (hir::ExprKind::Range(Box::new(s), Box::new(e)), Type::Range)
            }
            ast::Expr::Index(c, i) => {
                let c = self.check_expr(c, None)?;
                let i = self.check_expr(i, None)?;
                let ty = list_element(&c.ty).cloned().unwrap_or(Type::Any);
                (hir::ExprKind::Index(Box::new(c), Box::new(i)), ty)
            }
            ast::Expr::FieldAccess(lhs, field) => {
                let lhs_h = self.check_expr(lhs, None)?;
                let struct_name = match &lhs_h.ty {
                    Type::Struct(name) => name.clone(),
                    _ => {
                        return Err(semantic(
                            &self.file,
                            lhs.span,
                            2,
                            format!(
                                "Undefined variable: {}",
                                match &lhs.node {
                                    ast::Expr::Var(n) => n.clone(),
                                    _ => String::new(),
                                }
                            ),
                            None,
                        ));
                    }
                };
                let def = self.structs.get(&struct_name).ok_or_else(|| {
                    SprsError::Semantic {
                        code: ErrorCode {
                            category: ErrorCategory::Semantic,
                            number: 13,
                        },
                        location: Location::new(String::new(), Span::DUMMY),
                        message: format!("Undefined struct : {}", struct_name),
                        help: None,
                    }
                })?;
                let field_index = *def.field_indices.get(field).ok_or_else(|| {
                    semantic(
                        &self.file,
                        span,
                        2,
                        format!("Undefined variable: {}", field),
                        None,
                    )
                })?;
                let field_ty = def.fields[field_index as usize].ty.clone();
                (
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(lhs_h),
                        field_name: field.clone(),
                        struct_name,
                        field_index,
                    },
                    field_ty,
                )
            }
            ast::Expr::StructInit(struct_name, fields) => {
                let fields_h = self.check_struct_init(struct_name, fields)?;
                (
                    hir::ExprKind::StructInit {
                        struct_name: struct_name.clone(),
                        fields: fields_h,
                    },
                    Type::Struct(struct_name.clone()),
                )
            }
            ast::Expr::Atom(name) => {
                self.check_label_name(name, span)?;
                (hir::ExprKind::Atom(name.clone()), self.infer_type(expr))
            }
            ast::Expr::Label(name, payload) => {
                self.check_label_name(name, span)?;
                let payload_h = self.check_expr(payload, None)?;
                let ty = self.infer_type(expr);
                (hir::ExprKind::Label(name.clone(), Box::new(payload_h)), ty)
            }
            ast::Expr::AttachSlot(slot) => {
                if !self.attachments.contains(slot) {
                    return Err(semantic(
                        &self.file,
                        span,
                        2,
                        format!("attach slot '<:{}' used before @attach", slot),
                        None,
                    ));
                }
                (hir::ExprKind::AttachSlot(slot.clone()), Type::Any)
            }
            ast::Expr::Try(inner) => {
                let inner_h = self.check_expr(inner, None)?;
                let ty = inner_h.ty.clone();
                (hir::ExprKind::Try(Box::new(inner_h)), ty)
            }
            ast::Expr::Match { scrutinee, arms } => {
                let scrut_h = self.check_expr(scrutinee, None)?;
                self.validate_match_patterns(arms.iter().map(|a| (&a.pat, a.span)), &scrut_h.ty)?;
                self.check_closed_exhaustiveness(&scrut_h.ty, arms.iter().map(|a| (&a.pat, a.span)))?;
                let mut arms_h = Vec::new();
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    if let MatchPat::LabelPayload { binder, .. } = &arm.pat {
                        if binder != "_" {
                            self.bind(
                                binder.clone(),
                                Binding {
                                    ty: Type::Any,
                                    is_ambi: false,
                                    is_annotated: false,
                                },
                            );
                        }
                    }
                    let value = self.check_expr(&arm.value, None)?;
                    self.scopes.pop();
                    arms_h.push(hir::ExprMatchArm {
                        pat: arm.pat.clone(),
                        value,
                        span: arm.span,
                    });
                }
                let ty = self.infer_type(expr);
                (
                    hir::ExprKind::Match {
                        scrutinee: Box::new(scrut_h),
                        arms: arms_h,
                    },
                    ty,
                )
            }
            ast::Expr::HeapAlloc(size) => {
                let size = self.check_expr(size, None)?;
                (hir::ExprKind::HeapAlloc(Box::new(size)), Type::Buffer)
            }
            ast::Expr::Destroy(inner) => {
                let inner = self.check_expr(inner, None)?;
                (hir::ExprKind::Destroy(Box::new(inner)), Type::Unit)
            }
            ast::Expr::Exist(inner) => {
                let inner = self.check_expr(inner, None)?;
                (hir::ExprKind::Exist(Box::new(inner)), Type::Bool)
            }
        };
        let (kind, ty) = kind_ty;
        Ok(hir::Expr { kind, ty, span })
    }

    fn bin(
        &mut self,
        l: &Spanned<ast::Expr>,
        r: &Spanned<ast::Expr>,
        ctor: fn(Box<hir::Expr>, Box<hir::Expr>) -> hir::ExprKind,
    ) -> Result<(hir::ExprKind, Type), SprsError> {
        let l = self.check_expr(l, None)?;
        let r = self.check_expr(r, None)?;
        let ty = infer_binary_arith_type(&l.ty, &r.ty);
        Ok((ctor(Box::new(l), Box::new(r)), ty))
    }

    fn bin_bool(
        &mut self,
        l: &Spanned<ast::Expr>,
        r: &Spanned<ast::Expr>,
        ctor: fn(Box<hir::Expr>, Box<hir::Expr>) -> hir::ExprKind,
    ) -> Result<(hir::ExprKind, Type), SprsError> {
        let l = self.check_expr(l, None)?;
        let r = self.check_expr(r, None)?;
        Ok((ctor(Box::new(l), Box::new(r)), Type::Bool))
    }

    fn check_args(&mut self, args: &[Spanned<ast::Expr>]) -> Result<Vec<hir::Expr>, SprsError> {
        args.iter().map(|a| self.check_expr(a, None)).collect()
    }

    fn check_named_assign(
        &mut self,
        name: &str,
        rhs: &Spanned<ast::Expr>,
        span: Span,
    ) -> Result<hir::Expr, SprsError> {
        let target = self.get_binding(name).ok_or_else(|| {
            semantic(&self.file, span, 2, format!("Undefined variable: {}", name), None)
        })?;
        if let ast::Expr::Var(src) = &rhs.node {
            if src == name {
                return self.check_expr(rhs, Some(&target.ty));
            }
        }
        let rhs_h = self.check_expr_in(rhs, Some(&target.ty))?;
        if target.is_annotated && !target.is_ambi {
            self.check_list_literal_elements(rhs, &target.ty)?;
            if !types_assignable(&target.ty, &rhs_h.ty) {
                return Err(type_err(
                    &self.file,
                    span,
                    6,
                    format!(
                        "Type mismatch: cannot assign {} to fixed binding `{}` of type {}",
                        rhs_h.ty, name, target.ty
                    ),
                    Some(format!("{}", target.ty)),
                    Some(format!("{}", rhs_h.ty)),
                    Some(
                        "use `>> ambi T` if this parameter should allow dynamic reassignment"
                            .to_string(),
                    ),
                ));
            }
        }
        if !target.is_annotated || target.is_ambi {
            self.set_binding_type(name, rhs_h.ty.clone());
        }
        Ok(rhs_h)
    }

    fn check_struct_init(
        &mut self,
        struct_name: &str,
        field_exprs: &[(String, Spanned<ast::Expr>)],
    ) -> Result<Vec<(u32, hir::Expr)>, SprsError> {
        let def = self.structs.get(struct_name).ok_or_else(|| SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: format!("Undefined struct : {}", struct_name),
            help: None,
        })?;
        for (field_name, field_expr) in field_exprs {
            if !def.field_indices.contains_key(field_name) {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 13,
                    },
                    location: Location::new(String::new(), field_expr.span),
                    message: format!("unknown field `{}` in init {}", field_name, struct_name),
                    help: Some("fields must match the struct declaration".to_string()),
                });
            }
        }
        for (idx, (field_name, field_expr)) in field_exprs.iter().enumerate() {
            if field_exprs[..idx].iter().any(|(prev, _)| prev == field_name) {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 13,
                    },
                    location: Location::new(String::new(), field_expr.span),
                    message: format!("duplicate field `{}` in init {}", field_name, struct_name),
                    help: Some("each field may be initialized at most once".to_string()),
                });
            }
        }
        for field in &def.fields {
            let has_explicit = field_exprs.iter().any(|(name, _)| name == &field.name);
            if !has_explicit && field.default_value.is_none() {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 13,
                    },
                    location: Location::new(String::new(), field.span),
                    message: format!(
                        "missing required field `{}` in init {}",
                        field.name, struct_name
                    ),
                    help: Some(
                        "provide a value or add a default to the field declaration".to_string(),
                    ),
                });
            }
        }
        let fields_meta = def.fields.clone();
        let indices = def.field_indices.clone();
        let mut out = Vec::new();
        for field in &fields_meta {
            let index = indices[&field.name];
            let expr = if let Some((_, e)) = field_exprs.iter().find(|(n, _)| n == &field.name) {
                self.check_expr(e, Some(&field.ty))?
            } else {
                field.default_value.clone().expect("validated")
            };
            out.push((index, expr));
        }
        Ok(out)
    }

    fn check_macro(
        &mut self,
        ident: &str,
        args: &[Spanned<ast::Expr>],
        span: Span,
    ) -> Result<(hir::ExprKind, Type), SprsError> {
        match ident {
            "raw" | "free" if self.unsafe_depth == 0 => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 15,
                    },
                    location: Location::new(
                        String::new(),
                        args.first().map(|a| a.span).unwrap_or(Span::DUMMY),
                    ),
                    message: format!("`@{ident}` requires an unsafe block"),
                    help: Some("wrap the call in `unsafe { ... }`".to_string()),
                });
            }
            _ => {}
        }
        let arity_err = |n: usize, msg: String, number: u32| {
            if args.len() != n {
                Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number,
                    },
                    location: Location::new(String::new(), Span::DUMMY),
                    message: msg,
                    help: None,
                })
            } else {
                Ok(())
            }
        };
        match ident {
            "list_push" => {
                arity_err(2, "list_push expects 2 arguments".into(), 13)?;
                let list_ty = self.infer_type(&args[0]);
                if let Some(elem_ty) = list_element(&list_ty) {
                    if !matches!(elem_ty, Type::Any) {
                        let val_ty = self.infer_type_in(&args[1], Some(elem_ty));
                        if !types_assignable(elem_ty, &val_ty) {
                            return Err(self.type_mismatch_assign(
                                args[1].span,
                                format!("Type mismatch: list element has {val_ty}, expected {elem_ty}"),
                                elem_ty,
                                &val_ty,
                            ));
                        }
                    }
                }
            }
            "cast" => {
                if args.len() != 2 {
                    return Err(semantic(&self.file, span, 13, "@cast expects 2 arguments".into(), None));
                }
            }
            "is_error" => arity_err(1, "@is_error expects exactly 1 argument".into(), 3)?,
            "attach" => {
                arity_err(2, "@attach expects exactly 2 arguments: expression and label".into(), 3)?;
                match &args[1].node {
                    ast::Expr::AttachSlot(name) => {
                        self.attachments.insert(name.clone());
                    }
                    _ => {
                        return Err(SprsError::Semantic {
                            code: ErrorCode {
                                category: ErrorCategory::Semantic,
                                number: 3,
                            },
                            location: Location::new(String::new(), args[1].span),
                            message: "@attach second argument must be a slot such as <:name".into(),
                            help: None,
                        });
                    }
                }
            }
            "label_is" => {
                arity_err(2, "@label_is expects exactly 2 arguments: value and label".into(), 3)?;
            }
            "raw" => arity_err(1, "@raw expects 1 argument".into(), 13)?,
            "free" => arity_err(1, "@free expects 1 argument".into(), 13)?,
            "println" | "buf_len" | "buf_get" | "buf_set" | "clone" | "move" | "fcast"
            | "lshift" | "rshift" | "not" | "error_message" | "label_payload" | "label_name"
            | "error" => {}
            _ => {
                return Err(semantic(
                    &self.file,
                    span,
                    3,
                    format!("Unknown macro: {}", ident),
                    None,
                ));
            }
        }
        let args_h = self.check_args(args)?;
        let mut ty = self.macro_result_type(ident, args);
        if ident == "cast" && args.len() >= 2 {
            ty = args_h[1].ty.clone();
        }
        Ok((hir::ExprKind::Macro(ident.to_string(), args_h), ty))
    }

    fn validate_match_patterns<'a>(
        &self,
        arms: impl Iterator<Item = (&'a MatchPat, Span)>,
        scrut_ty: &Type,
    ) -> Result<(), SprsError> {
        let is_atom_static = matches!(scrut_ty, Type::App(name, args) if name == "Atom" && args.len() == 1)
            || matches!(scrut_ty, Type::AtomVal | Type::ClosedLabelSet(_));
        let arms: Vec<_> = arms.collect();
        let last = arms.len().saturating_sub(1);
        for (i, (pat, span)) in arms.iter().enumerate() {
            match pat {
                MatchPat::Name(LabelName::Dynamic(_))
                | MatchPat::LabelPayload {
                    name: LabelName::Dynamic(_),
                    ..
                } => {
                    return Err(semantic(
                        &self.file,
                        *span,
                        17,
                        "match patterns must be static :name in v1".into(),
                        Some("dynamic :\"{i}-item\" patterns are not supported yet".into()),
                    ));
                }
                MatchPat::LabelPayload { .. } if is_atom_static => {
                    return Err(semantic(
                        &self.file,
                        *span,
                        17,
                        "payload pattern requires Label scrutinee".into(),
                        Some("use a plain :name pattern for Atom values".into()),
                    ));
                }
                MatchPat::Name(LabelName::Static(name)) => {
                    self.resolve_closed_label_member(name, *span)?;
                }
                MatchPat::Wildcard if i != last => {
                    return Err(semantic(
                        &self.file,
                        *span,
                        17,
                        "case _ must be the last match arm".into(),
                        Some("move case _ to the end".into()),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_closed_exhaustiveness<'a>(
        &self,
        scrut_ty: &Type,
        arms: impl Iterator<Item = (&'a MatchPat, Span)>,
    ) -> Result<(), SprsError> {
        let Type::ClosedLabelSet(set) = scrut_ty else {
            return Ok(());
        };
        let Some((members, _)) = self.closed_label_sets.get(set) else {
            return Ok(());
        };
        if members.is_empty() {
            return Ok(());
        }
        let mut covered: HashSet<&str> = HashSet::new();
        let mut first_span: Option<Span> = None;
        for (pat, span) in arms {
            if first_span.is_none() {
                first_span = Some(span);
            }
            match pat {
                MatchPat::Wildcard => return Ok(()),
                MatchPat::Name(LabelName::Static(name)) => {
                    covered.insert(name.as_str());
                }
                _ => {}
            }
        }
        let missing: Vec<String> = members
            .iter()
            .map(|m| format!("{}.{}", set, m))
            .filter(|full| !covered.contains(full.as_str()))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(semantic(
            &self.file,
            first_span.unwrap_or(Span::DUMMY),
            17,
            format!("non-exhaustive match on {}; missing {}", set, missing.join(", ")),
            None,
        ))
    }

    fn check_function(
        &mut self,
        func: &ast::Function,
        known_structs: &HashSet<String>,
        known_closed: &HashSet<String>,
    ) -> Result<hir::Function, SprsError> {
        self.attachments.clear();
        self.scopes.push(HashMap::new());
        if let Some(ret) = &func.ret_ty {
            reject_payloadless_label_type(ret).map_err(|msg| {
                semantic(&self.file, func.span, 11, msg, None)
            })?;
        }
        let mut ret_ty = func.ret_ty.clone();
        if let Some(ty) = &mut ret_ty {
            resolve_type(ty, known_structs, known_closed, None).map_err(|message| {
                semantic(&self.file, func.span, 11, message, None)
            })?;
        }
        self.current_fn_ret_ty = ret_ty;
        let mut params = Vec::new();
        for p in &func.params {
            let mut annot = p.ty.clone();
            if let Some(a) = &mut annot {
                resolve_type(&mut a.ty, known_structs, known_closed, None).map_err(|message| {
                    semantic(&self.file, p.span, 11, message, None)
                })?;
                reject_payloadless_label_type(&a.ty).map_err(|message| {
                    semantic(&self.file, p.span, 11, message, None)
                })?;
            }
            let (ty, is_ambi, is_annotated) = match annot {
                Some(a) => (a.ty, a.ambi, true),
                None => (Type::Any, false, false),
            };
            self.bind(
                p.ident.clone(),
                Binding {
                    ty: ty.clone(),
                    is_ambi,
                    is_annotated,
                },
            );
            params.push(hir::FunctionParam {
                name: p.ident.clone(),
                ty,
                is_ambi,
                is_annotated,
                span: p.span,
            });
        }
        let body = self.check_block(&func.blk)?;
        self.scopes.pop();
        self.attachments.clear();
        let (type_params, when_rules) = match &func.build_ref {
            Some(name) => self
                .function_build_contracts
                .get(name)
                .cloned()
                .unwrap_or_default(),
            None => (Vec::new(), Vec::new()),
        };
        let ret_ty = self.current_fn_ret_ty.take();
        Ok(hir::Function {
            name: func.ident.clone(),
            params,
            body,
            ret_ty,
            is_public: func.is_public,
            type_params,
            when_rules,
            span: func.span,
        })
    }

    fn check_block(&mut self, stmts: &[Spanned<ast::Stmt>]) -> Result<Vec<hir::Stmt>, SprsError> {
        self.scopes.push(HashMap::new());
        let mut out = Vec::new();
        for stmt in stmts {
            out.push(self.check_stmt(stmt)?);
        }
        self.scopes.pop();
        Ok(out)
    }

    fn check_stmt(&mut self, stmt: &Spanned<ast::Stmt>) -> Result<hir::Stmt, SprsError> {
        let span = stmt.span;
        let kind = match &stmt.node {
            ast::Stmt::Var(var) => {
                let mut annot = var.ty.clone();
                if let Some(a) = &mut annot {
                    let known_structs: HashSet<String> = self.structs.keys().cloned().collect();
                    let known_closed: HashSet<String> = self.closed_label_sets.keys().cloned().collect();
                    resolve_type(&mut a.ty, &known_structs, &known_closed, None).map_err(|message| {
                        semantic(&self.file, var.span, 11, message, None)
                    })?;
                    reject_payloadless_label_type(&a.ty).map_err(|message| {
                        semantic(&self.file, var.span, 11, message, None)
                    })?;
                }
                let expected_ty = annot.as_ref().map(|a| &a.ty);
                let unit = Spanned::new(ast::Expr::Unit(), Span::DUMMY);
                let init_ast = var.expr.as_ref().unwrap_or(&unit);
                let init = self.check_expr_in(init_ast, expected_ty)?;
                if let Some(expected) = expected_ty {
                    if !types_assignable(expected, &init.ty) {
                        return Err(self.type_mismatch_assign(
                            var.span,
                            format!(
                                "Type mismatch: cannot assign {} to fixed binding `{}` of type {}",
                                init.ty, var.ident, expected
                            ),
                            expected,
                            &init.ty,
                        ));
                    }
                    self.check_list_literal_elements(init_ast, expected)?;
                }
                let binding_ty = match &annot {
                    Some(a) => a.ty.clone(),
                    None => init.ty.clone(),
                };
                let (is_ambi, is_annotated) = match &annot {
                    Some(a) => (a.ambi, true),
                    None => (false, false),
                };
                self.bind(
                    var.ident.clone(),
                    Binding {
                        ty: binding_ty.clone(),
                        is_ambi,
                        is_annotated,
                    },
                );
                hir::StmtKind::Var {
                    name: var.ident.clone(),
                    binding_ty,
                    is_ambi,
                    is_annotated,
                    init,
                }
            }
            ast::Stmt::Return(expr_opt) => {
                let expr_h = match expr_opt {
                    Some(e) => {
                        let expected = self.current_fn_ret_ty.clone();
                        let h = self.check_expr_in(e, expected.as_ref())?;
                        if let Some(expected) = &expected {
                            self.check_list_literal_elements(e, expected)?;
                        }
                        if let Some(expected_ty) = &expected {
                            let actual = h.ty.clone();
                            if actual != Type::Any
                                && !is_error_label_type(&actual)
                                && !types_assignable(expected_ty, &actual)
                            {
                                return Err(type_err(
                                    &self.file,
                                    e.span,
                                    5,
                                    format!(
                                        "Type mismatch: Function declares >> {} but return expression has {}",
                                        expected_ty, actual
                                    ),
                                    Some(format!("{}", expected_ty)),
                                    Some(format!("{}", actual)),
                                    None,
                                ));
                            }
                        }
                        Some(h)
                    }
                    None => None,
                };
                hir::StmtKind::Return(expr_h)
            }
            ast::Stmt::If {
                cond,
                then_blk,
                else_blk,
            } => {
                let cond = self.check_expr(cond, None)?;
                let then_blk = self.check_block(then_blk)?;
                let else_blk = match else_blk {
                    Some(b) => Some(self.check_block(b)?),
                    None => None,
                };
                hir::StmtKind::If {
                    cond,
                    then_blk,
                    else_blk,
                }
            }
            ast::Stmt::While { cond, body } => {
                let cond = self.check_expr(cond, None)?;
                let body = self.check_block(body)?;
                hir::StmtKind::While { cond, body }
            }
            ast::Stmt::Unsafe { body, .. } => {
                self.unsafe_depth += 1;
                let result = self.check_block(body);
                self.unsafe_depth -= 1;
                hir::StmtKind::Unsafe { body: result? }
            }
            ast::Stmt::Defer { expr, .. } => hir::StmtKind::Defer {
                expr: self.check_expr(expr, None)?,
            },
            ast::Stmt::Match {
                scrutinee,
                bind,
                arms,
                ..
            } => {
                let scrut = self.check_expr(scrutinee, None)?;
                self.validate_match_patterns(arms.iter().map(|a| (&a.pat, a.span)), &scrut.ty)?;
                self.check_closed_exhaustiveness(&scrut.ty, arms.iter().map(|a| (&a.pat, a.span)))?;
                if let Some(name) = bind {
                    self.bind(
                        name.clone(),
                        Binding {
                            ty: Type::Unit,
                            is_ambi: false,
                            is_annotated: false,
                        },
                    );
                }
                let mut arms_h = Vec::new();
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    if let MatchPat::LabelPayload { binder, .. } = &arm.pat {
                        if binder != "_" {
                            self.bind(
                                binder.clone(),
                                Binding {
                                    ty: Type::Any,
                                    is_ambi: false,
                                    is_annotated: false,
                                },
                            );
                        }
                    }
                    let body = match &arm.body {
                        ast::MatchArmBody::ExprBreak(e) => {
                            if let Some(name) = bind {
                                let rhs = self.check_named_assign(name, e, arm.span)?;
                                hir::MatchArmBody::ExprBreak(rhs)
                            } else {
                                hir::MatchArmBody::ExprBreak(self.check_expr(e, None)?)
                            }
                        }
                        ast::MatchArmBody::Block(stmts) => {
                            hir::MatchArmBody::Block(self.check_block(stmts)?)
                        }
                    };
                    self.scopes.pop();
                    arms_h.push(hir::MatchArm {
                        pat: arm.pat.clone(),
                        body,
                        span: arm.span,
                    });
                }
                hir::StmtKind::Match {
                    scrutinee: scrut,
                    bind: bind.clone(),
                    arms: arms_h,
                }
            }
            ast::Stmt::Expr(expr) => hir::StmtKind::Expr(self.check_expr(expr, None)?),
            ast::Stmt::Assign(assign) => {
                let rhs = self.check_named_assign(&assign.name, &assign.expr, assign.span)?;
                hir::StmtKind::Assign {
                    name: assign.name.clone(),
                    rhs,
                }
            }
            ast::Stmt::IndexAssign {
                collection,
                index,
                expr,
                span: ia_span,
            } => {
                let coll = self.check_expr(collection, None)?;
                if let Some(elem_ty) = list_element(&coll.ty) {
                    if !matches!(elem_ty, Type::Any) {
                        let rhs_ty = self.infer_type_in(expr, Some(elem_ty));
                        if !types_assignable(elem_ty, &rhs_ty) {
                            return Err(self.type_mismatch_assign(
                                *ia_span,
                                format!("Type mismatch: list element has {rhs_ty}, expected {elem_ty}"),
                                elem_ty,
                                &rhs_ty,
                            ));
                        }
                    }
                }
                let index = self.check_expr(index, None)?;
                let expr = self.check_expr(expr, None)?;
                hir::StmtKind::IndexAssign {
                    collection: coll,
                    index,
                    expr,
                }
            }
        };
        Ok(hir::Stmt { kind, span })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::error::SprsError;
    use crate::front::parser::parse_only;

    fn check(src: &str) -> Result<hir::Module, SprsError> {
        let items = parse_only(src, "test.sprs").expect("parse");
        check_module(&items, "test", "test.sprs", &HashMap::new(), &HashMap::new())
    }

    fn first_fn(module: &hir::Module) -> &hir::Function {
        &module.functions[0]
    }

    #[test]
    fn typed_list_push_and_index() {
        let src = r#"
fn f() >> i64 {
    var xs >> List(i64) = [];
    @list_push(xs, 1);
    return xs[0];
}
"#;
        let module = check(src).expect("check");
        let body = &first_fn(&module).body;
        let hir::StmtKind::Var { init, binding_ty, .. } = &body[0].kind else {
            panic!("expected var");
        };
        assert_eq!(*binding_ty, Type::App("List".into(), vec![Type::TypeI64]));
        assert_eq!(init.ty, Type::App("List".into(), vec![Type::TypeI64]));
        let hir::StmtKind::Expr(push) = &body[1].kind else {
            panic!("expected push");
        };
        assert_eq!(push.ty, Type::Unit);
        let hir::StmtKind::Return(Some(ret)) = &body[2].kind else {
            panic!("expected return");
        };
        assert_eq!(ret.ty, Type::TypeI64);
    }

    #[test]
    fn cast_target_is_hir_type() {
        let src = r#"
fn f() >> i16 {
    return @cast(1, i16);
}
"#;
        let module = check(src).expect("check");
        let hir::StmtKind::Return(Some(expr)) = &first_fn(&module).body[0].kind else {
            panic!("return");
        };
        assert_eq!(expr.ty, Type::TypeI16);
        let hir::ExprKind::Macro(name, args) = &expr.kind else {
            panic!("macro");
        };
        assert_eq!(name, "cast");
        assert_eq!(args[1].ty, Type::TypeI16);
    }

    fn err_code(err: &SprsError) -> (ErrorCategory, u32) {
        match err {
            SprsError::Type { code, .. } => (code.category, code.number),
            SprsError::Semantic { code, .. } => (code.category, code.number),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn negative_table() {
        let cases: &[(&str, ErrorCategory, u32)] = &[
            (
                r#"fn f() >> i64 { return "x"; }"#,
                ErrorCategory::Type,
                5,
            ),
            (
                r#"fn f(){ var x >> i64 = "x"; }"#,
                ErrorCategory::Type,
                6,
            ),
            (
                r#"fn f(){ var xs >> List(i64) = [1, "x"]; }"#,
                ErrorCategory::Type,
                6,
            ),
            (
                r#"fn g(x >> i64) {} fn f(){ g("x"); }"#,
                ErrorCategory::Type,
                7,
            ),
            (
                r#"fn f() { match :ok { case {:ok, x} => {} } }"#,
                ErrorCategory::Semantic,
                17,
            ),
        ];
        for (src, cat, num) in cases {
            let err = check(src).expect_err(src);
            assert_eq!(err_code(&err), (*cat, *num), "{src}");
        }
    }

    #[test]
    fn dynamic_label_part_type() {
        let src = r#"
fn f() {
    var xs >> List(i64) = [];
    var a = :"{xs}-x";
}
"#;
        let err = check(src).expect_err("dyn");
        assert_eq!(err_code(&err), (ErrorCategory::Semantic, 3));
    }

    #[test]
    fn resolves_forward_struct_and_self() {
        let module = check("struct A { b >> B } struct B { a >> A }").expect("check");
        assert_eq!(module.structs[0].fields[0].ty, Type::Struct("B".into()));
        assert_eq!(module.structs[1].fields[0].ty, Type::Struct("A".into()));
        let module = check("struct Node { next >> Self, children >> List(Self) }").expect("check");
        assert_eq!(module.structs[0].fields[0].ty, Type::Struct("Node".into()));
        assert_eq!(
            module.structs[0].fields[1].ty,
            Type::App("List".into(), vec![Type::Struct("Node".into())])
        );
    }

    #[test]
    fn rejects_undefined_named_type() {
        let err = check("struct A { value >> DoesNotExist }").expect_err("undef");
        assert_eq!(err_code(&err), (ErrorCategory::Semantic, 11));
        match err {
            SprsError::Semantic { message, location, .. } => {
                assert_eq!(message, "Undefined type: DoesNotExist");
                assert_eq!(location.file, "test.sprs");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_self_outside_struct() {
        let err = check("fn f(x >> Self) {}").expect_err("self");
        assert_eq!(err_code(&err), (ErrorCategory::Semantic, 11));
        match err {
            SprsError::Semantic { message, .. } => {
                assert_eq!(
                    message,
                    "`Self` is only valid in struct field type annotations"
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
