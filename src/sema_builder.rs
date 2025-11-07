use crate::ast;
use crate::type_helper::Type;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ItemSig {
    pub name: String,
    pub ix: usize,
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
    items
        .iter()
        .enumerate()
        .map(|(ix, item)| ItemSig {
            name: item.ident.clone(),
            ix,
        })
        .collect()
}

// Collect all variable declarations from all items
pub fn collect_all_vardecls<'a>(
    items: &'a [ast::Item],
    sigs: &[ItemSig],
) -> Vec<(&'a str, &'a ast::VarDecl)> {
    let mut out = Vec::new();
    for sig in sigs {
        let item = &items[sig.ix];
        collect_vardecls_in_block(&item.blk, &item.ident, &mut out);
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
        }
    }
}

pub fn build_var_table<'a>(items: &'a [ast::Item], sigs: &[ItemSig]) -> VarTable<'a> {
    let mut var_table: VarTable<'a> = HashMap::new();
    for sig in sigs {
        let item = &items[sig.ix];
        let mut rows = Vec::new();
        collect_varinfo_in_block(&item.blk, &mut rows);
        var_table.insert(&item.ident, rows);
    }
    var_table
}

fn collect_varinfo_in_block<'a>(stmts: &'a [ast::Stmt], table: &mut Vec<VarInfo<'a>>) {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Var(var) => table.push(VarInfo {
                decl: var,
                ty_hint: infer_type_hint(&var.expr).unwrap_or(Type::Any),
            }),
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
        }
    }
}

fn infer_type_hint(expr: &ast::Expr) -> Option<Type> {
    use ast::Expr::*;
    match expr {
        Number(_) => Some(Type::Int),
        Add(left, right) | Mul(left, right) => {
            match (infer_type_hint(left), infer_type_hint(right)) {
                (Some(Type::Int), Some(Type::Int)) => Some(Type::Int),
                _ => None,
            }
        }
        Eq(_, _) => Some(Type::Bool),
        If(_, then_expr, else_expr) => {
            let then_type = infer_type_hint(then_expr)?;
            let else_type = infer_type_hint(else_expr)?;
            if then_type == else_type {
                Some(then_type)
            } else {
                None
            }
        }
    }
}
