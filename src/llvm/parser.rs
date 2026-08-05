use crate::front::ast;
use crate::front::error::SprsError;
use crate::front::error_reporter;
use crate::front::lexer;
use crate::grammar;

pub fn parse_only(input: &str, file_path: &str) -> Result<Vec<ast::Item>, SprsError> {
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => Ok(items),
        Err(e) => Err(error_reporter::to_sprs_error(input, file_path, e)),
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
fn f(xs >> list) >> list { return xs; }
fn g(r >> range) >> range { return r; }
fn h() >> err { return @error(100, "x"); }
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
            Item::FunctionItem(f) => f.params[0].ty.as_ref(),
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
fn f(xs >> List(int)) >> List(int) { return xs; }
fn g() >> Result(int, err) { return 1; }
fn h(xs >> List()) >> list { return xs; }
"#;
        let items = parse_only(src, "test.sprs").expect("parse");

        let list_int = Type::App("List".into(), vec![Type::Int]);
        let result_int_err = Type::App(
            "Result".into(),
            vec![Type::Int, Type::App("Label".into(), vec![Type::Atom("error".into())])],
        );

        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&list_int));
                assert_eq!(
                    f.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: list_int.clone(),
                        ambi: false
                    })
                );
            }
            other => panic!("expected f, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&result_int_err));
            }
            other => panic!("expected g, got {:?}", other),
        }
        match &items[2] {
            Item::FunctionItem(f) => {
                assert_eq!(
                    f.ret_ty.as_ref(),
                    Some(&Type::List),
                    "flat list keyword still parses"
                );
                assert_eq!(
                    f.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::App("List".into(), vec![]),
                        ambi: false
                    })
                );
            }
            other => panic!("expected h, got {:?}", other),
        }
    }

    #[test]
    fn parses_nested_app_type_annotation() {
        let src = "fn f(x >> List(Result(int, err))) { return; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        let expected = Type::App(
            "List".into(),
            vec![Type::App(
                "Result".into(),
                vec![Type::Int, Type::App("Label".into(), vec![Type::Atom("error".into())])],
            )],
        );
        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(f.params[0].ty.as_ref().map(|a| &a.ty), Some(&expected));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn parses_ambi_param_annotation() {
        let src = "fn f(a >> ambi int, b >> int) { a = \"x\"; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(
                    f.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::Int,
                        ambi: true
                    })
                );
                assert_eq!(
                    f.params[1].ty.as_ref(),
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
        let src = "fn f() >> list { return []; }\nfn main() { var x = f(); }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[1] {
            Item::FunctionItem(f) => {
                let stmt = &f.blk[0];
                match &stmt.node {
                    crate::front::ast::Stmt::Var(v) => {
                        let init = v.expr.as_ref().unwrap();
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
        let src = "fn main() { var a = :ok; var b = {:value, 42}; }\n";
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
        let src = r#"fn main() { var a = :"{i}-item"; var b = {:"{i}-item", 1}; }"#;
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
                        crate::front::label_name::LabelNamePart::Ident("i".into()),
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
                        crate::front::label_name::LabelNamePart::Ident("i".into()),
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
        assert!(parse_only(r#"fn main() { var a = :"{i+1}"; }"#, "bad1.sprs").is_err());
        assert!(parse_only(r#"fn main() { var a = :"{}"; }"#, "bad2.sprs").is_err());
    }

    #[test]
    fn parses_label_type_annotations() {
        let src = r#"
fn f(x >> label) >> label { return x; }
fn g(x >> Label(int)) >> Label(int) { return x; }
"#;
        let items = parse_only(src, "label_ty.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&Type::Label));
                assert_eq!(
                    f.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::Label,
                        ambi: false
                    })
                );
            }
            other => panic!("expected f, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(f) => {
                let expected = Type::App("Label".into(), vec![Type::Int]);
                assert_eq!(f.ret_ty.as_ref(), Some(&expected));
            }
            other => panic!("expected g, got {:?}", other),
        }
    }

    #[test]
    fn parses_named_label_type_annotations() {
        let src = r#"
fn f() >> Label(:ok) { return :ok; }
fn g(x >> Label(:ok, int)) >> Label(:ok, int) { return x; }
fn h() >> err { return :error; }
"#;
        let items = parse_only(src, "named_label_ty.sprs").expect("parse");
        let label_ok = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        let label_ok_int =
            Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);
        let err_sugar = Type::App("Label".into(), vec![Type::Atom("error".into())]);

        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&label_ok));
            }
            other => panic!("expected f, got {:?}", other),
        }
        match &items[1] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&label_ok_int));
                assert_eq!(
                    f.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: label_ok_int.clone(),
                        ambi: false
                    })
                );
            }
            other => panic!("expected g, got {:?}", other),
        }
        match &items[2] {
            Item::FunctionItem(f) => {
                assert_eq!(f.ret_ty.as_ref(), Some(&err_sugar));
            }
            other => panic!("expected h, got {:?}", other),
        }
    }

    #[test]
    fn parses_named_label_inside_app_type_annotation() {
        let src = "fn f(x >> List(Label(:ok, str))) { return; }\n";
        let items = parse_only(src, "named_label_nested.sprs").expect("parse");
        let expected = Type::App(
            "List".into(),
            vec![Type::App(
                "Label".into(),
                vec![Type::Atom("ok".into()), Type::Str],
            )],
        );
        match &items[0] {
            Item::FunctionItem(f) => {
                assert_eq!(f.params[0].ty.as_ref().map(|a| &a.ty), Some(&expected));
            }
            other => panic!("expected function, got {:?}", other),
        }
    }
}
