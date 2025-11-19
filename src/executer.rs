use crate::ast::{self, Function};
use std::collections::{HashMap, hash_map};

type Scope = HashMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Unit => write!(f, "()"),
        }
    }
}

impl std::ops::Add for Value {
    type Output = Value;

    fn add(self, other: Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            _ => Value::Unit, // Simplified for demo purposes
        }
    }
}

impl std::ops::Mul for Value {
    type Output = Value;

    fn mul(self, other: Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a * b),
            _ => Value::Unit, // Simplified for demo purposes
        }
    }
}

pub fn execute(items: &Vec<ast::Item>, entry_idx: usize) -> Result<(), String> {
    let mut functions = HashMap::new();

    for item in items {
        match item {
            ast::Item::FunctionItem(func) => {
                functions.insert(func.ident.as_str(), func);
            }
            _ => { /* Ignore other items for now */ }
        }
    }

    let entry_item = &items[entry_idx];
    match entry_item {
        ast::Item::FunctionItem(func) => match call_function(func, &[], &functions) {
            Ok(var) => {
                println!("Program finished with return value: {}", var);
                ()
            }
            Err(e) => return Err(format!("Error executing function {}: {}", func.ident, e)),
        },
        _ => return Err("Entry item is not a function".to_string()),
    }

    println!("Executing {} items...", items.len());
    Ok(())
}

fn call_function(
    func: &Function,
    arg_value: &[Value],
    functions: &HashMap<&str, &Function>,
) -> Result<Value, String> {
    println!("Calling function: {}", func.ident);

    let body = &func.blk;
    let args = &func.params;

    let mut scope: Scope = HashMap::new();

    for (idx, param) in args.iter().enumerate() {
        let val = arg_value.get(idx).cloned().unwrap_or(Value::Unit);
        scope.insert(param.ident.clone(), val.clone());
        println!("  Param {}: {} = {}", idx, param.ident, val);
    }

    execute_block(body, functions, &mut scope)
}

fn execute_block(
    stmts: &Vec<ast::Stmt>,
    functions: &HashMap<&str, &Function>,
    scope: &mut Scope,
) -> Result<Value, String> {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Var(var) => {
                let val = evalute_expr(&var.expr, functions, scope)?;
                scope.insert(var.ident.clone(), val.clone());
                println!("  Declared variable {}: {}", val, var.ident);
            }
            ast::Stmt::Expr(expr) => {
                println!("  Evaluating expression: {:?}", expr);
                match evalute_expr(expr, functions, scope) {
                    Ok(val) => println!("    Result: {}", val),
                    Err(e) => return Err(format!("Error evaluating expression: {}", e)),
                }
            }
            ast::Stmt::If {
                cond,
                then_blk,
                else_blk,
            } => {}
            ast::Stmt::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    println!("  Evaluating return expression: {:?}", expr);
                    match evalute_expr(expr, functions, scope) {
                        Ok(val) => {
                            println!("    Return value: {}", val);
                            return Ok(val);
                        }
                        Err(e) => return Err(format!("Error evaluating return expression: {}", e)),
                    }
                } else {
                    println!("  Return with no value");
                    return Ok(Value::Unit);
                }
            }
        }
    }
    Ok(Value::Unit)
}

fn evalute_expr(
    expr: &ast::Expr,
    functions: &HashMap<&str, &Function>,
    scope: &Scope,
) -> Result<Value, String> {
    match expr {
        ast::Expr::Number(n) => Ok(Value::Int(*n)),
        ast::Expr::Add(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(left + right)
        }
        ast::Expr::Mul(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(left * right)
        }
        ast::Expr::Eq(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(Value::Bool(left == right))
        }
        ast::Expr::If(cond, then_expr, else_expr) => {
            let condition = evalute_expr(cond, functions, scope)?;
            if condition != Value::Int(0) {
                evalute_expr(then_expr, functions, scope)
            } else {
                evalute_expr(else_expr, functions, scope)
            }
        }
        ast::Expr::Call(ident, args, _) => {
            // builtin functions
            if ident == "print" {
                for arg in args {
                    let val = evalute_expr(arg, functions, scope)?;
                    println!("{}", val);
                }
                return Ok(Value::Unit);
            }

            if let Some(func) = functions.get(ident.as_str()) {
                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(evalute_expr(arg, functions, scope)?);
                }
                call_function(func, &arg_values, functions)
            } else {
                Err(format!("Function {} not found", ident))
            }
        }
        ast::Expr::Var(ident) => {
            if let Some(val) = scope.get(ident) {
                println!("  Accessing variable {}: {}", val, ident);
                Ok(val.clone())
            } else {
                Err(format!("Variable {} not found", ident))
            }
        }
        _ => Err("Not implemented".to_string()),
    }
}
