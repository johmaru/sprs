use crate::executer::Value;

pub fn builtin_function_push(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("push function requires 2 arguments".to_string());
    }
    let list = &args[0];
    let value = &args[1];

    match list {
        Value::List(rc_refcell) => {
            let mut vec = rc_refcell.borrow_mut();
            vec.push(value.clone());
            Ok(Value::Unit)
        }
        _ => Err("First argument to push must be a list".to_string()),
    }
}
