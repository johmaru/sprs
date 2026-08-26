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
    use crate::front::ast::{
        AtomDef, ClosedLabelSet, Expr, FbCondition, Function, FunctionBuild,
        FunctionBuildDirective, Item, MatchArmBody, MatchPat, Stmt, Struct,
    };
    use crate::front::label_name::LabelName;
    use crate::front::type_helper::{Type, TypeAnnot};

    #[test]
    fn parses_list_range_error_type_annotations() {
        let src = r#"
fn first_function(xs >> List(Any)) >> List(Any) { return xs; }
fn second_function(range_input >> Range) >> Range { return range_input; }
fn error_value() >> Label(:error, Any) { return @error(100, "x"); }
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
                &Type::App("List".into(), vec![Type::Any]),
                &Type::Range,
                &Type::App("Label".into(), vec![Type::Atom("error".into()), Type::Any])
            ]
        );

        let list_param = match &items[0] {
            Item::FunctionItem(function_item) => function_item.params[0].ty.as_ref(),
            _ => None,
        };
        assert_eq!(
            list_param,
            Some(&TypeAnnot {
                ty: Type::App("List".into(), vec![Type::Any]),
                ambi: false
            })
        );
    }

    #[test]
    fn parses_app_type_annotations() {
        let src = r#"
fn first_function(xs >> List(i64)) >> List(i64) { return xs; }
fn second_function(job >> Process(str)) >> Process(str) { return job; }
fn third_function(xs >> List(Any)) >> List(Any) { return xs; }
"#;
        let items = parse_only(src, "test.sprs").expect("parse");

        let list_int = Type::App("List".into(), vec![Type::TypeI64]);
        let process_str = Type::App("Process".into(), vec![Type::Str]);

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
                assert_eq!(function_item.ret_ty.as_ref(), Some(&process_str));
            }
            other => panic!("expected second function, got {:?}", other),
        }
        match &items[2] {
            Item::FunctionItem(function_item) => {
                assert_eq!(
                    function_item.ret_ty.as_ref(),
                    Some(&Type::App("List".into(), vec![Type::Any])),
                    "List(Any) is the canonical untyped list spelling"
                );
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::App("List".into(), vec![Type::Any]),
                        ambi: false
                    })
                );
            }
            other => panic!("expected third function, got {:?}", other),
        }
    }

    #[test]
    fn parses_nested_app_type_annotation() {
        let src = "fn nested_value(input_value >> List(Process(i64))) { return; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        let expected = Type::App(
            "List".into(),
            vec![Type::App("Process".into(), vec![Type::TypeI64])],
        );
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(
                    function_item.params[0]
                        .ty
                        .as_ref()
                        .map(|annotation| &annotation.ty),
                    Some(&expected)
                );
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn parses_ambi_param_annotation() {
        let src = "fn dynamic_parameter(first_value >> ambi i64, second_value >> i64) { first_value = \"x\"; }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(
                    function_item.params[0].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::TypeI64,
                        ambi: true
                    })
                );
                assert_eq!(
                    function_item.params[1].ty.as_ref(),
                    Some(&TypeAnnot {
                        ty: Type::TypeI64,
                        ambi: false
                    })
                );
            }
            other => panic!("expected function, got {:?}", other),
        }
    }

    #[test]
    fn call_expr_has_no_embedded_ret_ty() {
        let src = "fn list_factory() >> List(Any) { return []; }\nfn main() { var list_result = list_factory(); }\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        match &items[1] {
            Item::FunctionItem(function_item) => {
                let stmt = &function_item.blk[0];
                match &stmt.node {
                    crate::front::ast::Stmt::Var(variable_statement) => {
                        let init = variable_statement.expr.as_ref().unwrap();
                        assert!(matches!(
                            init.node,
                            crate::front::ast::Expr::Call { .. }
                        ));
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
            Some(crate::front::ast::Expr::Atom(
                crate::front::label_name::LabelName::Static(name)
            )) if name == "ok"
        ));
        let crate::front::ast::Stmt::Var(second) = &function.blk[1].node else {
            panic!("expected second variable");
        };
        assert!(matches!(
            second.expr.as_ref().map(|expr| &expr.node),
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Static(name),
                payload
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
            Some(crate::front::ast::Expr::Atom(crate::front::label_name::LabelName::Dynamic(
                parts,
            ))) => {
                assert_eq!(
                    parts,
                    &vec![
                        crate::front::label_name::LabelNamePart::Ident("item_index".into()),
                        crate::front::label_name::LabelNamePart::Lit("-item".into()),
                    ]
                );
            }
            other => panic!("expected dynamic atom, got {:?}", other),
        }
        let crate::front::ast::Stmt::Var(second) = &function.blk[1].node else {
            panic!("expected second variable");
        };
        match second.expr.as_ref().map(|expr| &expr.node) {
            Some(crate::front::ast::Expr::Label(
                crate::front::label_name::LabelName::Dynamic(parts),
                payload,
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
    fn parses_attach_slot() {
        let src = "fn main() { var item = <:item; @attach(1, <:item); }\n";
        let items = parse_only(src, "attach_slot.sprs").expect("parse");
        let crate::front::ast::Item::FunctionItem(function) = &items[0] else {
            panic!("expected function");
        };
        let crate::front::ast::Stmt::Var(first) = &function.blk[0].node else {
            panic!("expected first variable");
        };
        assert!(matches!(
            first.expr.as_ref().map(|expr| &expr.node),
            Some(crate::front::ast::Expr::AttachSlot(name)) if name == "item"
        ));
    }

    #[test]
    fn rejects_invalid_dynamic_label_templates() {
        assert!(
            parse_only(
                r#"fn main() { var label_value = :"{item_index+1}"; }"#,
                "bad1.sprs"
            )
            .is_err()
        );
        assert!(parse_only(r#"fn main() { var label_value = :"{}"; }"#, "bad2.sprs").is_err());
    }

    #[test]
    fn parses_label_type_annotations() {
        let src = r#"
fn first_label_value(input_value >> Label) >> Label { return input_value; }
fn second_label_value(input_value >> Label(:ok, str)) >> Label(:ok, str) { return input_value; }
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
                let expected =
                    Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Str]);
                assert_eq!(function_item.ret_ty.as_ref(), Some(&expected));
            }
            other => panic!("expected second function, got {:?}", other),
        }
    }

    #[test]
    fn parses_named_label_type_annotations() {
        let src = r#"
fn first_function() >> :ok { return :ok; }
fn second_function(input_value >> Label(:ok, i64)) >> Label(:ok, i64) { return input_value; }
fn third_function() >> Label(:error, Any) { return :error; }
"#;
        let items = parse_only(src, "named_label_ty.sprs").expect("parse");
        let atom_ok = Type::Atom("ok".into());
        let label_ok_int =
            Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::TypeI64]);
        let err_label = Type::App("Label".into(), vec![Type::Atom("error".into()), Type::Any]);

        match &items[0] {
            Item::FunctionItem(function_item) => {
                assert_eq!(function_item.ret_ty.as_ref(), Some(&atom_ok));
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
                assert_eq!(function_item.ret_ty.as_ref(), Some(&err_label));
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
                assert_eq!(
                    function_item.params[0]
                        .ty
                        .as_ref()
                        .map(|annotation| &annotation.ty),
                    Some(&expected)
                );
            }
            other => panic!("expected function, got {:?}", other),
        }
    }
    #[test]
    fn parses_match_bind_ident_form() {
        let src = r#"
fn request_check(req >> Label(:ok, i64)) >> Label(:result, i64) {
    match req ?(var result) {
        case :ok => {:result, 0} break;
        case {:error, _reason} => {:result, 1} break;
    }
    return result;
}
"#;
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Match {
            scrutinee,
            bind,
            arms,
            ..
        } = &function_item.blk[0].node
        else {
            panic!("expected match statement");
        };
        assert_eq!(bind.as_deref(), Some("result"));
        assert!(matches!(&scrutinee.node, Expr::Var(name) if name == "req"));
        assert_eq!(arms.len(), 2);
        assert_eq!(arms[0].pat, MatchPat::Name(LabelName::Static("ok".into())));
        assert!(matches!(arms[0].body, MatchArmBody::ExprBreak(_)));
        assert_eq!(
            arms[1].pat,
            MatchPat::LabelPayload {
                name: LabelName::Static("error".into()),
                binder: "_reason".into(),
            }
        );
        assert!(matches!(arms[1].body, MatchArmBody::ExprBreak(_)));
    }

    #[test]
    fn parses_match_bind_parenthesized_expr() {
        let src = r#"
fn f() >> i64 {
    match (foo()) ?(var r) {
        case :ok => 1 break;
    }
    return r;
}
"#;
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Match {
            scrutinee,
            bind,
            arms,
            ..
        } = &function_item.blk[0].node
        else {
            panic!("expected match statement");
        };
        assert_eq!(bind.as_deref(), Some("r"));
        assert!(matches!(&scrutinee.node, Expr::Call { name, .. } if name == "foo"));
        assert_eq!(arms.len(), 1);
    }

    #[test]
    fn parses_match_no_bind_block_arms() {
        let src = r#"
fn f(req) {
    match req {
        case :ok => { return 1; }
        case :error => { return 0; }
    }
}
"#;
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Match { bind, arms, .. } = &function_item.blk[0].node else {
            panic!("expected match statement");
        };
        assert!(bind.is_none());
        assert_eq!(arms.len(), 2);
        assert!(matches!(arms[0].body, MatchArmBody::Block(_)));
        assert!(matches!(arms[1].body, MatchArmBody::Block(_)));
    }

    #[test]
    fn parses_label_payload_patterns_with_underscore() {
        let src = r#"
fn f(req) {
    match req {
        case {:ok, x} => { @println(x); }
        case {:ok, _} => { @println("ignored"); }
    }
}
"#;
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Match { arms, .. } = &function_item.blk[0].node else {
            panic!("expected match statement");
        };
        assert_eq!(
            arms[0].pat,
            MatchPat::LabelPayload {
                name: LabelName::Static("ok".into()),
                binder: "x".into(),
            }
        );
        assert_eq!(
            arms[1].pat,
            MatchPat::LabelPayload {
                name: LabelName::Static("ok".into()),
                binder: "_".into(),
            }
        );
    }

    #[test]
    fn parses_match_bind_label_literal_scrutinee() {
        let src = r#"
fn f() >> i64 {
    match {:ok, 7} ?(var r) {
        case :ok => 1 break;
    }
    return r;
}
"#;
        let items = parse_only(src, "t.sprs").expect("parse");
        assert!(!items.is_empty());
    }

    #[test]
    fn parses_match_bind_atom_literal_scrutinee() {
        let src = r#"
fn f() >> i64 {
    match :ok ?(var r) {
        case :ok => 1 break;
    }
    return r;
}
"#;
        let items = parse_only(src, "t.sprs").expect("parse");
        assert!(!items.is_empty());
    }

    #[test]
    fn parses_match_expression_form() {
        let src = r#"
fn f() >> i64 {
    var r = match :ok {
        case :ok => 1
        case :error => 0
        case _ => -1
    };
    return r;
}
"#;
        let items = parse_only(src, "t.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var statement");
        };
        let init = var_decl.expr.as_ref().expect("var has initializer");
        let Expr::Match { scrutinee, arms } = &init.node else {
            panic!("expected match expression");
        };
        assert!(matches!(
            &scrutinee.node,
            Expr::Atom(LabelName::Static(name)) if name == "ok"
        ));
        assert_eq!(arms.len(), 3);
        assert_eq!(arms[0].pat, MatchPat::Name(LabelName::Static("ok".into())));
        assert_eq!(
            arms[1].pat,
            MatchPat::Name(LabelName::Static("error".into()))
        );
        assert_eq!(arms[2].pat, MatchPat::Wildcard);
    }

    #[test]
    fn parses_match_wildcard_in_stmt_form() {
        let src = r#"
fn f() >> i64 {
    var flag = 0;
    match :ok {
        case :ok => { flag = 1; }
        case _ => { flag = -1; }
    }
    return flag;
}
"#;
        let items = parse_only(src, "t.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Match { arms, .. } = &function_item.blk[1].node else {
            panic!("expected match statement");
        };
        assert_eq!(arms.len(), 2);
        assert!(matches!(arms[0].body, MatchArmBody::Block(_)));
        assert_eq!(arms[1].pat, MatchPat::Wildcard);
    }

    #[test]
    fn rejects_bare_keywords_as_identifiers() {
        // Keywords can no longer be used as bare identifiers anywhere:
        // they are reserved and require the `^` escape.
        for source in [
            "pkg match;
",
            "fn if() {}
",
            "fn f(new) {}
",
            "fn f() { var defer = 1; }
",
            "fn f() { var return = 1; }
",
        ] {
            let error = parse_only(source, "keyword_idents.sprs")
                .expect_err("bare keyword identifier must be rejected");
            let message = format!("{error}");
            assert!(
                message.contains("SPRS-SYN-002") || message.contains("reserved"),
                "expected reserved-keyword diagnostic, got: {message}"
            );
            match error {
                crate::front::error::SprsError::Parse { help, .. } => {
                    let help = help.unwrap_or_default();
                    assert!(
                        help.contains("^"),
                        "expected `^` escape hint in help, got message={message} help={help}"
                    );
                }
                other => panic!("expected Parse error, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_unnecessary_escape() {
        // `^foo` (non-keyword) is an unnecessary escape (SYN-008).
        let error = parse_only("fn f() { var ^foo = 1; }\n", "escaped.sprs").unwrap_err();
        let message = format!("{error}");
        assert!(
            message.contains("SPRS-SYN-008") || message.contains("unnecessary identifier escape"),
            "unexpected error: {message}"
        );
        assert!(message.contains("^foo"), "missing name in: {message}");

        // `foo!` / `^foo!` are no longer valid identifiers.
        assert!(parse_only("fn f() { var foo! = 1; }\n", "bang.sprs").is_err());
        assert!(parse_only("fn f() { var ^foo! = 1; }\n", "bang2.sprs").is_err());
    }

    #[test]
    fn parses_escaped_keywords_as_idents() {
        let src = r#"
fn ^fn(^if) { var ^return = ^if; ^return = ^return + 1; return ^return; }
"#;
        let items = parse_only(src, "escaped_keyword_idents.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        assert_eq!(function_item.ident, "fn");
        assert_eq!(function_item.params[0].ident, "if");
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var statement");
        };
        assert_eq!(var_decl.ident, "return");
        let init = var_decl.expr.as_ref().expect("var has initializer");
        assert!(matches!(&init.node, Expr::Var(name) if name == "if"));
        let Stmt::Assign(assign) = &function_item.blk[1].node else {
            panic!("expected assign statement");
        };
        assert_eq!(assign.name, "return");
        assert!(matches!(
            &assign.expr.node,
            Expr::Add(lhs, rhs)
                if matches!(&lhs.node, Expr::Var(name) if name == "return")
                    && matches!(&rhs.node, Expr::Number(1))
        ));
        let Stmt::Return(Some(ret)) = &function_item.blk[2].node else {
            panic!("expected return statement");
        };
        assert!(matches!(&ret.node, Expr::Var(name) if name == "return"));
    }

    #[test]
    fn parses_canonical_buffer_and_rawptr_type_annotations() {
        let src = "fn f(x >> Buffer) >> RawPtr { return x; }";
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        assert_eq!(
            function_item.params[0].ty.as_ref().map(|t| &t.ty),
            Some(&Type::Buffer)
        );
        assert_eq!(function_item.ret_ty.as_ref(), Some(&Type::RawPtr));
    }

    #[test]
    fn new_call_still_heap_alloc() {
        let src = "fn f() { var a = new(4); }";
        let items = parse_only(src, "test.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var statement");
        };
        let init = var_decl.expr.as_ref().expect("var has initializer");
        assert!(matches!(init.node, Expr::HeapAlloc(_)));
    }

    #[test]
    fn parses_preprocessor_directive_before_comment_rule() {
        let src = "#define Windows\nfn main() {}\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        assert!(matches!(&items[0], Item::Preprocessor(name) if name == "Windows"));
        assert!(matches!(&items[1], Item::FunctionItem(_)));
    }

    #[test]
    fn hash_comments_are_skipped_without_define() {
        let src = "#comment\n# ordinary comment\nfn main() {}\n";
        let items = parse_only(src, "test.sprs").expect("parse");
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], Item::FunctionItem(_)));
    }

    #[test]
    fn parses_standalone_label_atom_def() {
        let src = "label :ready;\n";
        let items = parse_only(src, "label_atom.sprs").expect("parse");
        match &items[0] {
            Item::AtomItem(AtomDef {
                ident, is_public, ..
            }) => {
                assert_eq!(ident, "ready");
                assert!(!*is_public);
            }
            other => panic!("expected AtomItem, got {:?}", other),
        }
    }

    #[test]
    fn parses_pub_closed_label_set() {
        let src = "pub label Color { red, blue, }\n";
        let items = parse_only(src, "closed_label_set.sprs").expect("parse");
        match &items[0] {
            Item::ClosedLabelSetItem(ClosedLabelSet {
                ident,
                members,
                is_public,
                ..
            }) => {
                assert_eq!(ident, "Color");
                assert_eq!(members, &["red".to_string(), "blue".to_string()]);
                assert!(*is_public);
            }
            other => panic!("expected ClosedLabelSetItem, got {:?}", other),
        }
    }

    #[test]
    fn parses_qualified_atom_expression_and_match_pattern() {
        let src = r#"
fn f() {
    var x = :Color.red;
    var r = match x {
        case :Color.red => 1
        case _ => 0
    };
}
"#;
        let items = parse_only(src, "qualified_atom.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var");
        };
        assert!(matches!(
            var_decl.expr.as_ref().map(|expr| &expr.node),
            Some(Expr::Atom(LabelName::Static(name))) if name == "Color.red"
        ));
        let Stmt::Var(match_var) = &function_item.blk[1].node else {
            panic!("expected match var");
        };
        let init = match_var.expr.as_ref().expect("initializer");
        let Expr::Match { arms, .. } = &init.node else {
            panic!("expected match expression");
        };
        assert_eq!(
            arms[0].pat,
            MatchPat::Name(LabelName::Static("Color.red".into()))
        );
    }

    #[test]
    fn rejects_old_enum_and_old_grouped_label() {
        assert!(parse_only("enum Color { Red }\n", "old_enum.sprs").is_err());
        assert!(parse_only("label :Color{:red}\n", "old_grouped.sprs").is_err());
        assert!(parse_only("label :Color{:red, :blue}\n", "old_grouped_multi.sprs").is_err());
    }

    #[test]
    fn parses_enum_as_ordinary_identifier() {
        let src = r#"
fn enum(enum >> i64) {
    var enum = enum;
    return enum;
}
"#;
        let items = parse_only(src, "enum_ident.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        assert_eq!(function_item.ident, "enum");
        assert_eq!(function_item.params[0].ident, "enum");
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var");
        };
        assert_eq!(var_decl.ident, "enum");
    }

    #[test]
    fn rejects_empty_grouped_inner_and_old_var_label() {
        assert!(parse_only("label Color {}\n", "empty_grouped.sprs").is_err());
        assert!(parse_only("label :Color{}\n", "old_empty_grouped.sprs").is_err());
        assert!(parse_only("fn f() { label :red; }\n", "inner_label.sprs").is_err());
        assert!(parse_only("var Color = :Color{:red, :blue};\n", "old_var_label.sprs").is_err());
    }

    #[test]
    fn parses_value_and_expression_method_calls() {
        let items = parse_only(
            "fn main() { box.get(); factory().get(); }\n",
            "method_call.sprs",
        )
        .expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        match &function_item.blk[0].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::MemberCall {
                    receiver,
                    name,
                    type_args,
                    args,
                } => {
                    assert!(matches!(receiver.node, Expr::Var(ref ident) if ident == "box"));
                    assert_eq!(name, "get");
                    assert!(type_args.is_empty());
                    assert!(args.is_empty());
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
        match &function_item.blk[1].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::MemberCall {
                    receiver,
                    name,
                    args,
                    ..
                } => {
                    assert!(matches!(
                        receiver.node,
                        Expr::Call { ref name, .. } if name == "factory"
                    ));
                    assert_eq!(name, "get");
                    assert!(args.is_empty());
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_inline_generic_function_and_calls() {
        let items = parse_only(
            r#"
fn same<T>(left >> T, right >> T) >> T { return left; }
fn main() {
    same<i64>(1, 2);
    same<Pair(i64)>(p);
    fn_builds.fb_add(3, 4);
    a < b;
    a < i64;
}
"#,
            "generic_fn.sprs",
        )
        .expect("parse");
        let Item::FunctionItem(same) = &items[0] else {
            panic!("expected same");
        };
        assert_eq!(same.ident, "same");
        assert_eq!(same.type_params.len(), 1);
        assert_eq!(same.type_params[0].ident, "T");
        let Item::FunctionItem(main_fn) = &items[1] else {
            panic!("expected main");
        };
        match &main_fn.blk[0].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::Call { name, type_args, args } => {
                    assert_eq!(name, "same");
                    assert_eq!(type_args, &vec![Type::TypeI64]);
                    assert_eq!(args.len(), 2);
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
        match &main_fn.blk[1].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::Call { type_args, .. } => {
                    assert_eq!(
                        type_args,
                        &vec![Type::App("Pair".into(), vec![Type::TypeI64])]
                    );
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
        match &main_fn.blk[2].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::MemberCall { receiver, name, args, type_args } => {
                    assert!(matches!(receiver.node, Expr::Var(ref ident) if ident == "fn_builds"));
                    assert_eq!(name, "fb_add");
                    assert!(type_args.is_empty());
                    assert_eq!(args.len(), 2);
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
        match &main_fn.blk[3].node {
            Stmt::Expr(expr) => assert!(matches!(expr.node, Expr::Lt(_, _))),
            other => panic!("unexpected {other:?}"),
        }
        match &main_fn.blk[4].node {
            Stmt::Expr(expr) => assert!(matches!(expr.node, Expr::Lt(_, _))),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_nested_struct_methods() {
        let items = parse_only(
            r#"
struct MethodBox(T) {
    value >> T,
    pub fn get(self) >> T { return self.value; }
    fn set(self, next >> T) { }
}
"#,
            "methods.sprs",
        )
        .expect("parse");
        let Item::StructItem(st) = &items[0] else {
            panic!("expected struct");
        };
        assert_eq!(st.fields.len(), 1);
        assert_eq!(st.methods.len(), 2);
        assert_eq!(st.methods[0].ident, "get");
        assert!(st.methods[0].is_public);
        assert_eq!(st.methods[0].params[0].ident, "self");
        assert!(st.methods[0].params[0].ty.is_none());
        assert_eq!(st.methods[1].ident, "set");
        assert!(!st.methods[1].is_public);
    }

    #[test]
    fn parses_module_generic_call() {
        let items = parse_only(
            "fn main() { fn_builds.foo<i64>(x); }\n",
            "mod_generic.sprs",
        )
        .expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        match &function_item.blk[0].node {
            Stmt::Expr(expr) => match &expr.node {
                Expr::MemberCall { name, type_args, args, .. } => {
                    assert_eq!(name, "foo");
                    assert_eq!(type_args, &vec![Type::TypeI64]);
                    assert_eq!(args.len(), 1);
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_named_struct_and_self_types() {
        let items =
            parse_only("struct A {} fn f(x >> A) >> A { return x; }", "named.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[1] else {
            panic!("expected function");
        };
        assert_eq!(
            function_item.params[0].ty.as_ref().map(|annot| &annot.ty),
            Some(&Type::Named("A".into()))
        );
        assert_eq!(
            function_item.ret_ty.as_ref(),
            Some(&Type::Named("A".into()))
        );

        let items = parse_only("struct Node { next >> Self }", "self.sprs").expect("parse");
        let Item::StructItem(Struct { fields, .. }) = &items[0] else {
            panic!("expected struct");
        };
        assert_eq!(fields[0].ty.as_ref(), Some(&Type::SelfType));

        let items =
            parse_only("struct Node { children >> List(Self) }", "list_self.sprs").expect("parse");
        let Item::StructItem(Struct { fields, .. }) = &items[0] else {
            panic!("expected struct");
        };
        assert_eq!(
            fields[0].ty.as_ref(),
            Some(&Type::App("List".into(), vec![Type::SelfType]))
        );
    }

    #[test]
    fn parses_function_build_and_use() {
        let src = r#"
function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}

fn add use AddBuild {
    return lhs + rhs;
}
"#;
        let items = parse_only(src, "fb.sprs").expect("parse");
        match &items[0] {
            Item::FunctionBuildItem(FunctionBuild {
                ident,
                directives,
                is_public,
                ..
            }) => {
                assert_eq!(ident, "AddBuild");
                assert!(!*is_public);
                assert_eq!(directives.len(), 3);
                assert!(matches!(
                    &directives[0],
                    FunctionBuildDirective::Params { params, .. } if params.len() == 2
                ));
                assert!(matches!(
                    &directives[1],
                    FunctionBuildDirective::ReturnType { ty, .. } if *ty == Type::TypeI64
                ));
                assert!(matches!(
                    &directives[2],
                    FunctionBuildDirective::Visibility { is_public, .. } if *is_public
                ));
            }
            other => panic!("expected FunctionBuildItem, got {other:?}"),
        }
        let Item::FunctionItem(function) = &items[1] else {
            panic!("expected function");
        };
        assert_eq!(function.ident, "add");
        assert_eq!(function.build_ref.as_deref(), Some("AddBuild"));
        assert!(function.params.is_empty());
        assert_eq!(function.ret_ty, None);
        assert!(!function.is_public);
    }

    #[test]
    fn parses_function_build_with_type_param_and_when() {
        let src = r#"
function_build Identity {
    type_param T;
    params(value >> T);
    return_type(T);
    when T is i64 { return_type(i64); }
    when T is str and not T is bool { return_type(str); }
}
"#;
        let items = parse_only(src, "fb_cond.sprs").expect("parse");
        let Item::FunctionBuildItem(FunctionBuild { directives, .. }) = &items[0] else {
            panic!("expected function_build");
        };
        assert_eq!(directives.len(), 5);
        assert!(matches!(
            &directives[0],
            FunctionBuildDirective::TypeParam { ident, .. } if ident == "T"
        ));
        assert!(matches!(
            &directives[3],
            FunctionBuildDirective::When { condition, .. }
                if matches!(condition, FbCondition::Is { .. })
        ));
        assert!(matches!(
            &directives[4],
            FunctionBuildDirective::When { condition, .. }
                if matches!(condition, FbCondition::And { .. })
        ));
    }

    #[test]
    fn parses_pub_function_build() {
        let items = parse_only(
            "pub function_build PublicBuild {}
",
            "fb.sprs",
        )
        .expect("parse");
        let Item::FunctionBuildItem(FunctionBuild {
            is_public, ident, ..
        }) = &items[0]
        else {
            panic!("expected function_build");
        };
        assert_eq!(ident, "PublicBuild");
        assert!(*is_public);
    }

    #[test]
    fn parses_function_build_source_directive() {
        let items = parse_only(
            "function_build source contracts;
fn main() {}
",
            "fb.sprs",
        )
        .expect("parse");
        assert!(matches!(
            &items[0],
            Item::FunctionBuildSource { target, .. } if target == "contracts"
        ));
        assert!(matches!(&items[1], Item::FunctionItem(_)));
    }

    #[test]
    fn rejects_mixed_inline_and_function_build() {
        assert!(
            parse_only(
                "fn foo(x >> i64) use A {}
",
                "mix.sprs"
            )
            .is_err()
        );
        assert!(
            parse_only(
                "fn foo use A >> i64 {}
",
                "mix.sprs"
            )
            .is_err()
        );
        assert!(
            parse_only(
                "pub fn foo use A {}
",
                "mix.sprs"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_function_build_directive() {
        let err = parse_only(r#"function_build A { @println("x"); }"#, "bad.sprs").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("invalid FunctionBuild directive") || message.contains("Unrecognized"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn parses_init_struct_expression() {
        let src = "fn f() { var p = init Point { x = 1, y = 2, }; }\n";
        let items = parse_only(src, "init.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var statement");
        };
        let init = var_decl.expr.as_ref().expect("var has initializer");
        assert!(matches!(
            &init.node,
            Expr::StructInit { ty: Type::Named(name), fields }
                if name == "Point"
                    && fields.len() == 2
                    && fields[0].0 == "x"
                    && fields[1].0 == "y"
        ));
    }

    #[test]
    fn parses_init_empty_struct() {
        let src = "fn f() { var e = init Empty {}; }\n";
        let items = parse_only(src, "init_empty.sprs").expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var statement");
        };
        let init = var_decl.expr.as_ref().expect("var has initializer");
        assert!(matches!(&init.node, Expr::StructInit { ty: Type::Named(name), fields } if name == "Empty" && fields.is_empty()));
    }

    #[test]
    fn rejects_old_init_macro() {
        assert!(parse_only("fn f() { var p = @init(Point); }\n", "old_init.sprs").is_err());
    }

    #[test]
    fn rejects_cp_var() {
        assert!(parse_only("fn f() { cp var x = 1; }\n", "cp_var.sprs").is_err());
        assert!(parse_only("fn f() { cp var x = 1; var y = x; }\n", "cp_var2.sprs").is_err());
    }

    #[test]
    fn rejects_old_fb_directive_spellings() {
        assert!(parse_only(
            "function_build A { @FbArgs(x >> i64); }\n",
            "old_fb.sprs"
        )
        .is_err());
        assert!(parse_only(
            "function_build A { @FbRetTy(i64); }\n",
            "old_fb2.sprs"
        )
        .is_err());
        assert!(parse_only(
            "function_build A { @FbVisibility(pub); }\n",
            "old_fb3.sprs"
        )
        .is_err());
    }

    #[test]
    fn still_parses_normal_functions() {
        let items = parse_only(
            "pub fn add(lhs >> i64, rhs >> i64) >> i64 { return lhs + rhs; }
",
            "fn.sprs",
        )
        .expect("parse");
        let Item::FunctionItem(function) = &items[0] else {
            panic!("expected function");
        };
        assert!(function.is_public);
        assert!(function.build_ref.is_none());
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.ret_ty, Some(Type::TypeI64));
    }

    #[test]
    fn parses_var_type_annotation() {
        let src = "fn f() { var xs >> List(i64) = [1, 2, 3]; }\n";
        let items = parse_only(src, "var_ty.sprs").expect("parse");
        match &items[0] {
            Item::FunctionItem(function_item) => match &function_item.blk[0].node {
                crate::front::ast::Stmt::Var(var) => {
                    assert_eq!(
                        var.ty.as_ref().map(|a| &a.ty),
                        Some(&Type::App("List".into(), vec![Type::TypeI64]))
                    );
                }
                other => panic!("expected var, got {other:?}"),
            },
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn parses_unknown_user_type_constructor_as_app() {
        let items = parse_only(
            "fn f(x >> Result(i64, str)) { return; }\n",
            "user_ctor.sprs",
        )
        .expect("parse");
        let Item::FunctionItem(function_item) = &items[0] else {
            panic!("expected function");
        };
        let ty = function_item.params[0].ty.as_ref().map(|a| &a.ty);
        assert_eq!(
            ty,
            Some(&Type::App(
                "Result".into(),
                vec![Type::TypeI64, Type::Str]
            ))
        );
    }

    #[test]
    fn parses_generic_struct_decl_and_init() {
        let src = r#"
struct Pair(T) { a >> T, b >> T }
struct PairTwo(A, B) { a >> A, b >> B }
fn f() {
    var p = init Pair(i64) { a = 1, b = 2 };
}
"#;
        let items = parse_only(src, "generic.sprs").expect("parse");
        let Item::StructItem(pair) = &items[0] else {
            panic!("expected Pair struct");
        };
        assert_eq!(pair.ident, "Pair");
        assert_eq!(pair.type_params.len(), 1);
        assert_eq!(pair.type_params[0].ident, "T");
        assert!(pair.type_params[0].span.end > pair.type_params[0].span.start);
        let Item::StructItem(pair_two) = &items[1] else {
            panic!("expected PairTwo struct");
        };
        assert_eq!(
            pair_two
                .type_params
                .iter()
                .map(|p| p.ident.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
        let Item::FunctionItem(function_item) = &items[2] else {
            panic!("expected function");
        };
        let Stmt::Var(var_decl) = &function_item.blk[0].node else {
            panic!("expected var");
        };
        let init = var_decl.expr.as_ref().expect("init");
        match &init.node {
            Expr::StructInit { ty, fields } => {
                assert_eq!(ty, &Type::App("Pair".into(), vec![Type::TypeI64]));
                assert_eq!(fields.len(), 2);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_process_wrong_arity() {
        let err = parse_only("fn f(x >> Process(i64, str)) { return; }\n", "arity.sprs")
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("Process requires exactly one type argument"), "{msg}");
    }

    #[test]
    fn rejects_range_type_argument() {
        let err = parse_only("fn f(x >> Range(i64)) { return; }\n", "range.sprs").unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("Range does not take type arguments"), "{msg}");
    }

    #[test]
    fn parses_ptr_type_and_deref_place() {
        let src = r#"
fn read(p >> Ptr(i64)) >> i64 { return *p; }
fn nest(pp >> Ptr(Ptr(str))) >> str { return **pp; }
fn write(p >> Ptr(i64)) { *p = 7; }
"#;
        let items = parse_only(src, "ptr.sprs").expect("parse");
        let ptr_i64 = Type::App("Ptr".into(), vec![Type::TypeI64]);
        let ptr_ptr_str = Type::App(
            "Ptr".into(),
            vec![Type::App("Ptr".into(), vec![Type::Str])],
        );
        let Item::FunctionItem(read) = &items[0] else {
            panic!("expected read");
        };
        assert_eq!(read.ret_ty.as_ref(), Some(&Type::TypeI64));
        assert_eq!(
            read.params[0].ty.as_ref().map(|a| &a.ty),
            Some(&ptr_i64)
        );
        let Stmt::Return(Some(ret)) = &read.blk[0].node else {
            panic!("expected return *p");
        };
        match &ret.node {
            Expr::Deref(inner) => match &inner.node {
                Expr::Var(name) => assert_eq!(name, "p"),
                other => panic!("expected *p, got {other:?}"),
            },
            other => panic!("expected deref, got {other:?}"),
        }

        let Item::FunctionItem(nest) = &items[1] else {
            panic!("expected nest");
        };
        assert_eq!(nest.params[0].ty.as_ref().map(|a| &a.ty), Some(&ptr_ptr_str));
        let Stmt::Return(Some(ret)) = &nest.blk[0].node else {
            panic!("expected return **pp");
        };
        match &ret.node {
            Expr::Deref(outer) => match &outer.node {
                Expr::Deref(inner) => match &inner.node {
                    Expr::Var(name) => assert_eq!(name, "pp"),
                    other => panic!("expected **pp, got {other:?}"),
                },
                other => panic!("expected nested deref, got {other:?}"),
            },
            other => panic!("expected deref, got {other:?}"),
        }

        let Item::FunctionItem(write) = &items[2] else {
            panic!("expected write");
        };
        match &write.blk[0].node {
            Stmt::DerefAssign { pointer, expr, .. } => {
                assert!(matches!(&pointer.node, Expr::Var(name) if name == "p"));
                assert!(matches!(&expr.node, Expr::Number(7)));
            }
            other => panic!("expected *p = 7, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_ptr_arity() {
        for src in [
            "fn f(x >> Ptr) { return; }
",
            "fn f(x >> Ptr()) { return; }
",
            "fn f(x >> Ptr(i64, str)) { return; }
",
        ] {
            let err = parse_only(src, "ptr_arity.sprs").unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains("Ptr requires exactly one type argument"),
                "{src} => {msg}"
            );
        }
    }

    #[test]
    fn preserves_star_precedence() {
        let src = r#"
fn mul(a >> i64, b >> i64) >> i64 { return a * b; }
fn mul_deref(a >> i64, p >> Ptr(i64)) >> i64 { return a * *p; }
"#;
        let items = parse_only(src, "star.sprs").expect("parse");
        let Item::FunctionItem(mul) = &items[0] else {
            panic!("expected mul");
        };
        let Stmt::Return(Some(ret)) = &mul.blk[0].node else {
            panic!("expected return");
        };
        match &ret.node {
            Expr::Mul(lhs, rhs) => {
                assert!(matches!(&lhs.node, Expr::Var(name) if name == "a"));
                assert!(matches!(&rhs.node, Expr::Var(name) if name == "b"));
            }
            other => panic!("expected a * b, got {other:?}"),
        }
        let Item::FunctionItem(mul_deref) = &items[1] else {
            panic!("expected mul_deref");
        };
        let Stmt::Return(Some(ret)) = &mul_deref.blk[0].node else {
            panic!("expected return");
        };
        match &ret.node {
            Expr::Mul(lhs, rhs) => {
                assert!(matches!(&lhs.node, Expr::Var(name) if name == "a"));
                match &rhs.node {
                    Expr::Deref(inner) => {
                        assert!(matches!(&inner.node, Expr::Var(name) if name == "p"));
                    }
                    other => panic!("expected *p rhs, got {other:?}"),
                }
            }
            other => panic!("expected a * *p, got {other:?}"),
        }
    }
}
