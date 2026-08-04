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
        assert_eq!(tys, vec![&Type::List, &Type::Range, &Type::Error]);

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
}
