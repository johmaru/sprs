// interpreter currently not support yet, for now this file set a allowed unused
#![allow(unused)]

use crate::{
    front::ast::{self},
    interpreter::runner::parse_only,
};
use std::collections::HashMap;

type Scope = HashMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
    Return(Box<Value>),
    List(std::rc::Rc<std::cell::RefCell<Vec<Value>>>),
    Range(i64, i64),
    StructInit(String, HashMap<String, Value>),

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

pub struct Module {
    pub name: String,
    pub functions: HashMap<String, Callable<'static>>,
    pub variables: HashMap<String, Value>,
}
impl Module {
    pub fn new(name: &str) -> Self {
        Module {
            name: name.to_string(),
            functions: HashMap::new(),
            variables: HashMap::new(),
        }
    }
}
pub struct RuntimeContext<'a> {
    pub modules: HashMap<String, Module>,
    pub global_scope: Scope,
    pub functions: HashMap<&'a str, Callable<'a>>,
    pub program_data: ProgramSig,
}
impl<'a> RuntimeContext<'a> {
    pub fn new() -> Self {
        RuntimeContext {
            modules: HashMap::new(),
            global_scope: HashMap::new(),
            functions: HashMap::new(),
            program_data: ProgramSig {
                runtime_os: OS::Windows, // Default OS
            },
        }
    }
}

pub enum Callable<'a> {
    User(&'a ast::Function),
    Builtin(NativeFunction),
}

type NativeFunction = fn(&[Value]) -> Result<Value, String>;

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::TypeI8 => write!(f, "i8"),
            Value::TypeU8 => write!(f, "u8"),
            Value::TypeI16 => write!(f, "i16"),
            Value::TypeU16 => write!(f, "u16"),
            Value::TypeI32 => write!(f, "i32"),
            Value::TypeU32 => write!(f, "u32"),
            Value::TypeI64 => write!(f, "i64"),
            Value::TypeU64 => write!(f, "u64"),
            Value::TypeF16 => write!(f, "fp16"),
            Value::TypeF32 => write!(f, "fp32"),
            Value::TypeF64 => write!(f, "fp64"),
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
            Value::Range(start, end) => write!(f, "{}..{}", start, end),
            Value::StructInit(name, fields) => {
                write!(f, "{} {{ ", name)?;
                let mut first = true;
                for (key, value) in fields {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, value)?;
                    first = false;
                }
                write!(f, " }}")
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

pub struct ProgramSig {
    pub runtime_os: OS,
}

pub enum OS {
    Windows,
    Linux,
}

fn entry_builtin_functions() -> HashMap<&'static str, Callable<'static>> {
    let mut map = HashMap::new();
    map.insert(
        "println",
        Callable::Builtin(crate::runtime::builtin::builtin_function_println),
    );
    map.insert(
        "vec_push!",
        Callable::Builtin(crate::runtime::builtin::builtin_function_push),
    );
    map
}

fn load_modules(ctx: &mut RuntimeContext, module_name: &str) -> Result<(), String> {
    if ctx.modules.contains_key(module_name) {
        return Ok(());
    }

    let path = format!("{}.sprs", module_name);
    let source = std::fs::read_to_string(&path).map_err(|_| "file not found");
    let source = match source {
        Ok(s) => s,
        Err(e) => return Err(format!("Error reading module {}: {}", module_name, e)),
    };
    let ast = parse_only(&source, &path)
        .map_err(|e| format!("Error parsing module {}: {}", module_name, e));
    let ast = match ast {
        Ok(a) => a,
        Err(e) => return Err(format!("Error parsing module {}: {}", module_name, e)),
    };

    let mut preprocessors = Vec::new();
    for item in &ast {
        match item {
            ast::Item::Import(sub_module) => {
                load_modules(ctx, &sub_module).map_err(|e| format!("Error Load Module {}", e))?;
            }
            ast::Item::Preprocessor(pre) => {
                preprocessors.push(pre);
            }
            ast::Item::Import(ident) => {
                load_modules(ctx, ident)
                    .map_err(|e| format!("Error loading module {}: {}", ident, e))?;
            }
            ast::Item::Package(name) => {
                // Package declaration, can be used for namespacing if needed
                println!("Loading package: {}", name);
            }
            _ => {}
        }
    }

    execute_preprocessor(preprocessors, &mut ctx.program_data);

    let mut module = Module::new(module_name);

    ctx.modules.insert(module_name.to_string(), module);
    Ok(())
}

pub fn execute(
    ctx: &mut RuntimeContext,
    items: &[ast::Item],
    entry_idx: usize,
) -> Result<(), String> {
    let mut functions: HashMap<&str, Callable> = HashMap::new();
    let mut preprocessors = Vec::new();

    let mut builtins = entry_builtin_functions();
    functions.extend(builtins.drain());

    for item in items {
        match item {
            ast::Item::Import(module_name) => {
                load_modules(ctx, module_name)
                    .map_err(|e| format!("Error loading module {}: {}", module_name, e))?;
            }
            ast::Item::FunctionItem(func) => {
                functions.insert(func.ident.as_str(), Callable::User(func));
            }
            ast::Item::Preprocessor(pre) => {
                preprocessors.push(pre);
            }
            _ => { /* Ignore other items for now */ }
        }
    }

    execute_preprocessor(preprocessors, &mut ctx.program_data);

    let entry_item = &items[entry_idx];
    match entry_item {
        ast::Item::FunctionItem(func) => {
            match call_function(&Callable::User(func), &[], &functions) {
                Ok(var) => {
                    println!("Program finished with return value: {}", var);
                    ()
                }
                Err(e) => return Err(format!("Error executing function {}: {}", func.ident, e)),
            }
        }
        _ => return Err("Entry item is not a function".to_string()),
    }

    println!("Executing {} items...", items.len());
    Ok(())
}

fn execute_preprocessor(pre: Vec<&String>, program_data: &mut ProgramSig) {
    for directive in pre {
        if directive.starts_with("Windows") {
            program_data.runtime_os = OS::Windows;
        } else if directive.starts_with("Linux") {
            program_data.runtime_os = OS::Linux;
        } else {
            println!("Unknown preprocessor directive: {}", directive);
        }
    }
}

fn call_function(
    func: &Callable,
    arg_value: &[Value],
    functions: &HashMap<&str, Callable>,
) -> Result<Value, String> {
    match func {
        Callable::Builtin(builtin_fn) => builtin_fn(arg_value),
        Callable::User(func) => {
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
    }
}

fn execute_block(
    stmts: &Vec<ast::Stmt>,
    functions: &HashMap<&str, Callable>,
    scope: &mut Scope,
) -> Result<Value, String> {
    for stmt in stmts {
        match stmt {
            ast::Stmt::Var(var) => {
                let val = if let Some(expr) = &var.expr {
                    println!(
                        "  Evaluating variable declaration: {} = {:?}",
                        var.ident, expr
                    );
                    match evalute_expr(expr, functions, scope) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(format!("Error evaluating variable {}: {}", var.ident, e));
                        }
                    }
                } else {
                    println!("  Declaring variable {} with no initial value", var.ident);
                    Value::Unit
                };
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
            ast::Stmt::While { cond, body } => {
                println!("  Entering while loop with condition: {:?}", cond);
                loop {
                    match evalute_expr(&cond, functions, scope) {
                        Ok(val) => {
                            let is_true = match val {
                                Value::Bool(b) => b,
                                Value::Int(n) => n != 0,
                                _ => false,
                            };

                            if is_true {
                                println!("    Condition is true, executing loop body");
                                let result = execute_block(body, functions, scope)?;
                                if let Value::Return(_) = result {
                                    return Ok(result);
                                }
                            } else {
                                println!("    Condition is false, exiting loop");
                                break;
                            }
                        }
                        Err(e) => return Err(format!("Error evaluating while condition: {}", e)),
                    }
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
            ast::Stmt::EnumItem(enm) => {
                println!("  Enum declarations are not executed at runtime");
            }
            ast::Stmt::Assign(assign_stmt) => {
                println!(
                    "  Evaluating assignment: {} = {:?}",
                    assign_stmt.name, assign_stmt.expr
                );
                match evalute_expr(&assign_stmt.expr, functions, scope) {
                    Ok(val) => {
                        scope.insert(assign_stmt.name.clone(), val.clone());
                        println!("    Assigned variable {}: {}", assign_stmt.name, val);
                    }
                    Err(e) => {
                        return Err(format!(
                            "Error evaluating assignment for {}: {}",
                            assign_stmt.name, e
                        ));
                    }
                }
            }
        }
    }
    Ok(Value::Unit)
}

fn evalute_expr(
    expr: &ast::Expr,
    functions: &HashMap<&str, Callable>,
    scope: &Scope,
) -> Result<Value, String> {
    match expr {
        ast::Expr::Number(n) => Ok(Value::Int(*n)),
        ast::Expr::Float(f) => Ok(Value::Float(*f)),
        ast::Expr::TypeI8 => Ok(Value::TypeI8),
        ast::Expr::TypeU8 => Ok(Value::TypeU8),
        ast::Expr::TypeI16 => Ok(Value::TypeI16),
        ast::Expr::TypeU16 => Ok(Value::TypeU16),
        ast::Expr::TypeI32 => Ok(Value::TypeI32),
        ast::Expr::TypeU32 => Ok(Value::TypeU32),
        ast::Expr::TypeI64 => Ok(Value::TypeI64),
        ast::Expr::TypeU64 => Ok(Value::TypeU64),
        ast::Expr::TypeF16 => Ok(Value::TypeF16),
        ast::Expr::TypeF32 => Ok(Value::TypeF32),
        ast::Expr::TypeF64 => Ok(Value::TypeF64),
        ast::Expr::Str(s) => Ok(Value::Str(s.clone())),
        ast::Expr::Bool(b) => Ok(Value::Bool(*b)),
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
        ast::Expr::Mod(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
                _ => Err("Type error in modulus operation".to_string()),
            }
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
        ast::Expr::Lt(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
                _ => Err("Type error in less-than comparison".to_string()),
            }
        }
        ast::Expr::Gt(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
                _ => Err("Type error in greater-than comparison".to_string()),
            }
        }
        ast::Expr::Le(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
                _ => Err("Type error in less-than-or-equal comparison".to_string()),
            }
        }
        ast::Expr::Ge(lhs, rhs) => {
            let left = evalute_expr(lhs, functions, scope)?;
            let right = evalute_expr(rhs, functions, scope)?;
            match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
                _ => Err("Type error in greater-than-or-equal comparison".to_string()),
            }
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
        ast::Expr::Range(start_expr, end_expr) => {
            let start_val = evalute_expr(start_expr, functions, scope)?;
            let end_val = evalute_expr(end_expr, functions, scope)?;
            if let (Value::Int(start), Value::Int(end)) = (start_val, end_val) {
                Ok(Value::Range(start, end))
            } else {
                Err("Range bounds must be integers".to_string())
            }
        }
        ast::Expr::Index(collection_expr, index_expr) => {
            let collection_val = evalute_expr(collection_expr, functions, scope)?;
            let index_val = evalute_expr(index_expr, functions, scope)?;
            if let Value::List(ref elements_rc) = collection_val {
                let elements = elements_rc.borrow();
                if let Value::Int(index) = index_val {
                    let idx = index as usize;
                    if idx < elements.len() {
                        Ok(elements[idx].clone())
                    } else {
                        Err("Index out of bounds".to_string())
                    }
                } else {
                    Err("Index must be an integer".to_string())
                }
            } else {
                Err("Collection must be a list".to_string())
            }
        }
        ast::Expr::ModuleAccess(module_name, function_name, args) => {
            // !TODO Implement module access
            Err("Module access not implemented".to_string())
        }
        ast::Expr::FieldAccess(struct_expr, field_name) => {
            let struct_val = evalute_expr(struct_expr, functions, scope)?;
            // !TODO Implement field access for structs
            Err(format!(
                "Field access not implemented for field {}",
                field_name
            ))
        }
        ast::Expr::Unit() => Ok(Value::Unit),
        ast::Expr::StructInit(struct_name, fields) => {
            let mut field_values = HashMap::new();
            for (field_name, field_expr) in fields {
                let val = evalute_expr(field_expr, functions, scope)?;
                field_values.insert(field_name.clone(), val.clone());
                println!("  Initialized field {}: {}", field_name, val);
            }
            Ok(Value::StructInit(struct_name.clone(), field_values))
        }
    }
}
