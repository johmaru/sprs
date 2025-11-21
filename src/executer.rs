use crate::ast::{self, Function};
use std::collections::{HashMap, hash_map};

type Scope = HashMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Unit,
    Return(Box<Value>),
    List(std::rc::Rc<std::cell::RefCell<Vec<Value>>>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => write!(f, "{}", s),
            Value::Unit => write!(f, "()"),
            Value::Return(val) => write!(f, "Return({})", val),
            Value::List(elements) => {
                let value = elements.borrow();
                write!(f, "[")?;
                for (i, elem) in value.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
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

impl std::ops::Sub for Value {
    type Output = Value;

    fn sub(self, other: Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            _ => Value::Unit, // Simplified for demo purposes
        }
    }
}

impl std::ops::Div for Value {
    type Output = Value;

    fn div(self, other: Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
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

    let result = execute_block(body, functions, &mut scope)?;
    match result {
        Value::Return(val) => Ok(*val),
        _ => Ok(result),
    }
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
            } => {
                println!("  Evaluating if condition: {:?}", cond);
                match evalute_expr(cond, functions, scope) {
                    Ok(val) => {
                        let is_true = match val {
                            Value::Bool(b) => b,
                            Value::Int(n) => n != 0,
                            _ => false,
                        };

                        if is_true {
                            println!("    Condition is true, executing then block");
                            let result = execute_block(then_blk, functions, scope)?;
                            if let Value::Return(_) = result {
                                return Ok(result);
                            }
                        } else if let Some(else_block) = else_blk {
                            println!("    Condition is false, executing else block");
                            let result = execute_block(else_block, functions, scope)?;
                            if let Value::Return(_) = result {
                                return Ok(result);
                            }
                        } else {
                            println!("    Condition is false, no else block to execute");
                        }
                    }
                    Err(e) => return Err(format!("Error evaluating if condition: {}", e)),
                }
            }
            ast::Stmt::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    println!("  Evaluating return expression: {:?}", expr);
                    match evalute_expr(expr, functions, scope) {
                        Ok(val) => {
                            println!("    Return value: {}", val);
                            return Ok(Value::Return(Box::new(val)));
                        }
                        Err(e) => return Err(format!("Error evaluating return expression: {}", e)),
                    }
                } else {
                    println!("  Return with no value");
                    return Ok(Value::Return(Box::new(Value::Unit)));
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
        ast::Expr::Str(s) => Ok(Value::Str(s.clone())),
        ast::Expr::Add(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;

            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                _ => Err("Type error in addition".to_string()),
            }
        }
        ast::Expr::Mul(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(left * right)
        }
        ast::Expr::Minus(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(left - right)
        }
        ast::Expr::Div(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(left / right)
        }
        ast::Expr::Increment(expr) => {
            if let ast::Expr::Var(ident) = &**expr {
                if let Some(val) = scope.get(ident) {
                    if let Value::Int(n) = val {
                        let new_val = Value::Int(n + 1);
                        println!("  Incrementing variable {}: {} -> {}", ident, n, n + 1);
                        return Ok(new_val);
                    }
                }
                Err(format!("Variable {} not found or not an integer", ident))
            } else {
                Err("Increment operation requires a variable".to_string())
            }
        }
        ast::Expr::Decrement(expr) => {
            if let ast::Expr::Var(ident) = &**expr {
                if let Some(val) = scope.get(ident) {
                    if let Value::Int(n) = val {
                        let new_val = Value::Int(n - 1);
                        println!("  Decrementing variable {}: {} -> {}", ident, n, n - 1);
                        return Ok(new_val);
                    }
                }
                Err(format!("Variable {} not found or not an integer", ident))
            } else {
                Err("Decrement operation requires a variable".to_string())
            }
        }
        ast::Expr::Eq(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(Value::Bool(left == right))
        }
        ast::Expr::Neq(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            Ok(Value::Bool(left != right))
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

            if ident == "vec_push!" {
                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(evalute_expr(arg, functions, scope)?);
                    println!("  vec_push! argument: {}", arg_values.last().unwrap());
                }
                return crate::builtin::builtin_function_push(&arg_values);
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
        ast::Expr::List(elements) => {
            let mut list_values = Vec::new();
            for elem in elements {
                let val = evalute_expr(elem, functions, scope)?;
                list_values.push(val.clone());
                println!("  Added element to list: {}", val);
            }
            Ok(Value::List(std::rc::Rc::new(std::cell::RefCell::new(
                list_values,
            ))))
        }
    }
}
