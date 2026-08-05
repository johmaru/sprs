use crate::front::ast;
use crate::front::error::SprsError;
use crate::front::error_reporter;
use crate::front::lexer;
use crate::grammar;

pub fn parse_only(input: &str, file_path: &str) -> Result<Vec<ast::Item>, SprsError> {
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => Ok(items),
        Err(parse_error) => Err(error_reporter::to_sprs_error(input, file_path, parse_error)),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_only;
    use crate::front::ast::{Function, Item};
    use crate::front::type_helper::{Type, TypeAnnot};

    #[test]
    fn parses_list_range_error_type_annotations() {
        let src = r#"
fn first_function(xs >> list) >> list { return xs; }
fn second_function(range_input >> range) >> range { return range_input; }
fn error_value() >> err { return @error(100, "x"); }
"#;
        let items = parse_only(src, "test.sprs").expect("parse");
        let tys: Vec<&Type> = items
            .iter()
            .filter_map(|item| match item {
                Item::FunctionItem(Function {
                    ret_ty: Some(ty), ..
                }) => Some(ty),
                _ => None,
            })
            .collect();
        assert_eq!(
            tys,
            vec![
                &Type::List,
                &Type::Range,
                &Type::App("Label".into(), vec![Type::Atom("error".into())])
            ]
        );

        let list_param = match &items[0] {
            Item::FunctionItem(function_item) => function_item.params[0].ty.as_ref(),
            _ => None,
        };
        assert_eq!(
            list_param,
            Some(&TypeAnnot {
                ty: Type::List,
                ambi: false
            })
        );
    }

    #[test]
    fn parses_app_type_annotations() {
        let src = r#"
fn first_function(xs >> List(int)) >> List(int) { return xs; }
fn second_function() >> Result(int, err) { return 1; }
fn third_function(xs >> List()) >> list { return xs; }
"#;
        let items = parse_only(src, "test.sprs").expect("parse");

        let list_int = Type::App("List".into(), vec![Type::Int]);
        let result_int_err = Type::App(
            "Result".into(),
            vec![Type::Int, Type::App("Label".into(), vec![Type::Atom("error".into())])],
        );

        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&list_int));
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: list_int.clone(),
                        ambi: false
                    })
                );
            }
            other => panic!("expected first function, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&result_int_err));
            }
            other => panic!("expected second function, got {:?}", other),
        }
        match &items[2] {
            Item::FunctionItem(function_item) => {
                assert_eq!(
                    function_item.ret_ty.as_ref(),
                    Some(&Type::List),
                    "flat list keyword still parses"
                );
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::App("List".into(), vec![]),
                        ambi: false
                    })
                );
            }
            other => panic!("expected third function, got {:?}", other),
        }
    }

    #[test]
    fn parses_nested_app_type_annotation() {
        let src = "fn nested_value(input_value >> List(Result(int, err))) { return; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        let expected = Type::App(
            "List".into(),
            vec![Type::App(
                "Result".into(),
                vec![Type::Int, Type::App("Label".into(), vec![Type::Atom("error".into())])],
            )],
        );
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.params[0].ty.as_ref().map(|annotation| &annotation.ty), Some(&expected));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn parses_ambi_param_annotation() {
        let src = "fn dynamic_parameter(first_value >> ambi int, second_value >> int) { first_value = \"x\"; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::Int,
                        ambi: true
                    })
                );
                assert_eq!(
                    function_item.params[1].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::Int,
                        ambi: false
                    })
                );
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn call_expr_has_no_embedded_ret_ty() {
        let src = "fn list_factory() >> list { return []; }\nfn main() { var list_result = list_factory(); }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[1] {
            Item::FunctionItem(function_item) => {
                let stmt = &function_item.blk[0];
                match &stmt.node {
                    crate::front::ast::Stmt::Var(variable_statement) => {
                        let init = variable_statement.expr.as_ref().unwrap();
                        assert!(matches!(init.node, crate::front::ast::Expr::Call(_, _)));
                    }
                    other => panic!("expected var, got {:?}", other),
                }
            }
            other => panic!("expected main, got {:?}", other),
        }
    }
    #[test]
    fn parses_label_literals() {
        let src = "fn main() { var first_label = :ok; var second_label = {:value, 42}; }\n";
        let items = parse_only(src, "label.sprs").expect("parse");
        let crate::front::ast::Item::FunctionItem(function) = &items[0] else {
            panic!("expected function");
        };
        let crate::front::ast::Stmt::Var(first) = &function.blk[0].node else {
            panic!("expected first variable");
        };
        assert!(matches!(
            first.expr.as_ref().map(|expr| &expr.node),
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Static(name),
                None
            )) if name == "ok"
        ));
        let crate::front::ast::Stmt::Var(second) = &function.blk[1].node else {
            panic!("expected second variable");
        };
        assert!(matches!(
            second.expr.as_ref().map(|expr| &expr.node),
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Static(name),
                Some(payload)
            )) if name == "value" && matches!(payload.node, crate::front::ast::Expr::Number(42))
        ));
    }

    #[test]
    fn parses_dynamic_label_literals() {
        let src = r#"fn main() { var first_label = :"{item_index}-item"; var second_label = {:"{item_index}-item", 1}; }"#;
        let items = parse_only(src, "dyn_label.sprs").expect("parse");
        let crate::front::ast::Item::FunctionItem(function) = &items[0] else {
            panic!("expected function");
        };
        let crate::front::ast::Stmt::Var(first) = &function.blk[0].node else {
            panic!("expected first variable");
        };
        match first.expr.as_ref().map(|expr| &expr.node) {
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Dynamic(parts),
                None,
            )) => {
                assert_eq!(
                    parts,
                    &vec![
                        crate::front::label_name::LabelNamePart::Ident("item_index".into()),
                        crate::front::label_name::LabelNamePart::Lit("-item".into()),
                    ]
                );
            }
            other => panic!("expected dynamic label, got {:?}", other),
        }
        let crate::front::ast::Stmt::Var(second) = &function.blk[1].node else {
            panic!("expected second variable");
        };
        match second.expr.as_ref().map(|expr| &expr.node) {
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Dynamic(parts),
                Some(payload),
            )) => {
                assert_eq!(
                    parts,
                    &vec![
                        crate::front::label_name::LabelNamePart::Ident("item_index".into()),
                        crate::front::label_name::LabelNamePart::Lit("-item".into()),
                    ]
                );
                assert!(matches!(payload.node, crate::front::ast::Expr::Number(1)));
            }
            other => panic!("expected dynamic label with payload, got {:?}", other),
        }
    }

    #[test]
    fn rejects_invalid_dynamic_label_templates() {
        assert!(parse_only(r#"fn main() { var label_value = :"{item_index+1}"; }"#, "bad1.sprs").is_err());
        assert!(parse_only(r#"fn main() { var label_value = :"{}"; }"#, "bad2.sprs").is_err());
    }

    #[test]
    fn parses_label_type_annotations() {
        let src = r#"
fn first_label_value(input_value >> label) >> label { return input_value; }
fn second_label_value(input_value >> Label(int)) >> Label(int) { return input_value; }
"#;
        let items = parse_only(src, "label_ty.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&Type::Label));
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::Label,
                        ambi: false
                    })
                );
            }
            other => panic!("expected first function, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(function_item) => {
                let expected = Type::App("Label".into(), vec![Type::Int]);
                assert_eq!(function_item.ret_ty.as_ref(), Some(&expected));
            }
            other => panic!("expected second function, got {:?}", other),
        }
    }

    #[test]
    fn parses_named_label_type_annotations() {
        let src = r#"
fn first_function() >> Label(:ok) { return :ok; }
fn second_function(input_value >> Label(:ok, int)) >> Label(:ok, int) { return input_value; }
fn third_function() >> err { return :error; }
"#;
        let items = parse_only(src, "named_label_ty.sprs").expect("parse");
        let label_ok = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        let label_ok_int =
            Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);
        let err_sugar = Type::App("Label".into(), vec![Type::Atom("error".into())]);

        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&label_ok));
            }
            other => panic!("expected first function, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&label_ok_int));
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: label_ok_int.clone(),
                        ambi: false
                    })
                );
            }
            other => panic!("expected second function, got {:?}", other),
        }
        match &items[2] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&err_sugar));
            }
            other => panic!("expected third function, got {:?}", other),
        }
    }

    #[test]
    fn parses_named_label_inside_app_type_annotation() {
        let src = "fn nested_label_value(input_value >> List(Label(:ok, str))) { return; }\n";
        let items = parse_only(src, "named_label_nested.sprs").expect("parse");
        let expected = Type::App(
            "List".into(),
            vec![Type::App(
                "Label".into(),
                vec![Type::Atom("ok".into()), Type::Str],
            )],
        );
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.params[0].ty.as_ref().map(|annotation| &annotation.ty), Some(&expected));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }
}
