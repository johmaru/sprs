// interpreter currently not support yet, for now this file set a allowed unused
#![allow(unused)]

use crate::front::ast;
use crate::interpreter::type_helper::Type;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ItemSig {
    pub ty_: String,
    pub name: String,
    pub ix: usize,
    pub ret_ty: Type,
}

#[derive(Debug)]
pub struct FunctionSig {
    pub name: String,
    pub ix: usize,
    pub params: Vec<ast::FunctionParam>,
}

#[derive(Debug, Clone)]
pub struct VarInfo<'a> {
    pub decl: &'a ast::VarDecl,
    pub ty_hint: Type,
}

// Variable table mapping variable names to their info
pub type VarTable<'a> = HashMap<&'a str, Vec<VarInfo<'a>>>;

// Collect signatures of all items
pub fn collect_signatures(items: &[ast::Item]) -> Vec<ItemSig> {
    let mut sigs: Vec<ItemSig> = items
        .iter()
        .enumerate()
        .filter_map(|(ix, item)| match item {
            ast::Item::FunctionItem(func) => Some(ItemSig {
                ty_: "function".to_string(),
                name: func.ident.clone(),
                ix,
                ret_ty: Type::Any, // currently all any
            }),
            ast::Item::VarItem(_) => None,
            ast::Item::Preprocessor(_) => None,
            ast::Item::Import(_) => None,
            ast::Item::Package(_) => None,
        })
        .collect();

    let updates: Vec<(usize, Type)> = sigs
        .iter()
        .map(|sig| {
            let item = &items[sig.ix];
            if let ast::Item::FunctionItem(func) = item {
                let ret_ty = infer_return_type_from_block(&func.blk);
                (sig.ix, ret_ty)
            } else {
                (sig.ix, Type::Any)
            }
        })
        .collect();
    for (ix, ret_ty) in updates {
        if let Some(sig) = sigs.iter_mut().find(|s| s.ix == ix) {
            sig.ret_ty = ret_ty;
        }
    }
    sigs
}

// Collect all variable declarations from all items
pub fn collect_all_vardecls<'a>(
    items: &'a [ast::Item],
    sigs: &[ItemSig],
) -> Vec<(&'a str, &'a ast::VarDecl)> {
    let mut out = Vec::new();
    for sig in sigs {
        let item = &items[sig.ix];
        if let ast::Item::FunctionItem(func) = item {
            collect_vardecls_in_block(&func.blk, &func.ident, &mut out);
        }
    }
    out
}

pub fn collect_vardecls_in_block<'a>(
    stmts: &'a [ast::Stmt],
    item_name: &'a str,
    out: &mut Vec<(&'a str, &'a ast::VarDecl)>,
) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Var(var) => {
                out.push((item_name, var));
            }
            &ast::Stmt::Expr(_) => {}
            ast::Stmt::If {
                cond,
                then_blk,
                else_blk,
            } => {
                _ = cond; // ignore condition
                collect_vardecls_in_block(then_blk, item_name, out);
                if let Some(else_blk) = else_blk {
                    collect_vardecls_in_block(else_blk, item_name, out);
                }
            }
            ast::Stmt::While { cond, body } => {
                _ = cond; // ignore condition
                collect_vardecls_in_block(body, item_name, out);
            }
            ast::Stmt::Return(_) => {}
        }
    }
}

pub fn build_var_table<'a>(items: &'a [ast::Item], sigs: &[ItemSig]) -> VarTable<'a> {
    let mut var_table: VarTable<'a> = HashMap::new();
    for sig in sigs {
        let item = &items[sig.ix];
        if let ast::Item::FunctionItem(func) = item {
            let mut varinfos = Vec::new();
            collect_varinfo_in_block(&func.blk, &mut varinfos);
            var_table.insert(&func.ident, varinfos);
        }
    }
    var_table
}

fn collect_varinfo_in_block<'a>(stmts: &'a [ast::Stmt], table: &mut Vec<VarInfo<'a>>) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Var(var) => table.push(VarInfo {
                decl: var,
                ty_hint: infer_type_hint(&var.expr.as_ref().unwrap_or(&ast::Expr::Number(0)), &[])
                    .unwrap_or(Type::Any),
            }),
            ast::Stmt::Expr(_) => {}
            ast::Stmt::If {
                cond: _,
                then_blk,
                else_blk,
            } => {
                collect_varinfo_in_block(then_blk, table);
                if let Some(else_blk) = else_blk {
                    collect_varinfo_in_block(else_blk, table);
                }
            }
            ast::Stmt::While { cond: _, body } => {
                collect_varinfo_in_block(body, table);
            }
            ast::Stmt::Return(_) => {}
        }
    }
}

fn infer_return_type_from_block(stmts: &[ast::Stmt]) -> Type {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Return(Some(expr)) => {
                return infer_type_hint(&expr, &[]).unwrap_or(Type::Any);
            }
            ast::Stmt::If {
                then_blk, else_blk, ..
            } => {
                let then_ty = infer_return_type_from_block(then_blk);
                if then_ty != Type::Any {
                    return then_ty;
                }
                if let Some(blk) = else_blk {
                    let else_ty = infer_return_type_from_block(blk);
                    if else_ty != Type::Any {
                        return else_ty;
                    }
                }
            }
            _ => {}
        }
    }
    Type::Any
}

fn infer_type_hint(expr: &ast::Expr, sigs: &[ItemSig]) -> Option<Type> {
    use ast::Expr::*;
    match expr {
        Number(_) => Some(Type::Int),
        TypeI8 => Some(Type::TypeI8),
        TypeU8 => Some(Type::TypeU8),
        TypeI16 => Some(Type::TypeI16),
        TypeU16 => Some(Type::TypeU16),
        TypeI32 => Some(Type::TypeI32),
        TypeU32 => Some(Type::TypeU32),
        TypeI64 => Some(Type::TypeI64),
        TypeU64 => Some(Type::TypeU64),
        Bool(_) => Some(Type::Bool),
        Str(_) => Some(Type::Str),
        Var(_) => Some(Type::Any),
        Add(left, right) | Mul(left, right) => {
            match (infer_type_hint(left, sigs), infer_type_hint(right, sigs)) {
                (Some(Type::Int), Some(Type::Int)) => Some(Type::Int),
                _ => None,
            }
        }
        Minus(left, right) | Div(left, right) | Mod(left, right) => {
            match (infer_type_hint(left, sigs), infer_type_hint(right, sigs)) {
                (Some(Type::Int), Some(Type::Int)) => Some(Type::Int),
                _ => None,
            }
        }
        Increment(expr) | Decrement(expr) => infer_type_hint(expr, sigs),
        Eq(_, _) => Some(Type::Bool),
        Neq(_, _) => Some(Type::Bool),
        Lt(_, _) => Some(Type::Bool),
        Gt(_, _) => Some(Type::Bool),
        Le(_, _) => Some(Type::Bool),
        Ge(_, _) => Some(Type::Bool),
        If(_, then_expr, else_expr) => {
            let then_type = infer_type_hint(then_expr, sigs)?;
            let else_type = infer_type_hint(else_expr, sigs)?;
            if then_type == else_type {
                Some(then_type)
            } else {
                None
            }
        }
        Call(ident, args, ret_ty) => sigs
            .iter()
            .find(|sig| sig.name == *ident)
            .map(|sig| sig.ret_ty.clone()),
        List(_) => Some(Type::Any), // Currently, we treat all lists as Any type
        Range(_, _) => Some(Type::Any),
        Index(_, _) => Some(Type::Any),
        ModuleAccess(_, _, _) => Some(Type::Any), // !TODO Implement module access type inference
        Unit() => Some(Type::Unit),
    }
}
