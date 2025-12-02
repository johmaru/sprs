// auto-generated: "lalrpop 0.22.2"
// sha3: f1be3aba7d47cdd0caedd302980a8402dfc6542b0026cd05323881f7676eff5d
use crate::front::ast::{Item, VarDecl, Expr, Stmt, Function, FunctionParam};
use crate::front::lexer::Token;
use lalrpop_util::ParseError;
#[allow(unused_extern_crates)]
extern crate lalrpop_util as __lalrpop_util;
#[allow(unused_imports)]
use self::__lalrpop_util::state_machine as __state_machine;
#[allow(unused_extern_crates)]
extern crate alloc;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::match_single_binding)]
mod __parse__Start {

    use crate::front::ast::{Item, VarDecl, Expr, Stmt, Function, FunctionParam};
    use crate::front::lexer::Token;
    use lalrpop_util::ParseError;
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(Token),
        Variant1(Expr),
        Variant2(Vec<Expr>),
        Variant3(Vec<Stmt>),
        Variant4(bool),
        Variant5(()),
        Variant6(Item),
        Variant7(FunctionParam),
        Variant8(String),
        Variant9(alloc::vec::Vec<Item>),
        Variant10(Vec<FunctionParam>),
        Variant11(i64),
        Variant12(Vec<Item>),
        Variant13(Stmt),
        Variant14(alloc::vec::Vec<Stmt>),
        Variant15(Option<Expr>),
        Variant16(VarDecl),
        Variant17(Vec<VarDecl>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 52, 0, 6, 4, 5, 7,
        // State 1
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 2
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 52, 0, 6, 4, 5, 7,
        // State 3
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 4
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 5
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 6
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 7
        0, 0, 0, 0, 0, -49, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 8
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 9
        0, 0, 0, 0, 0, -41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0,
        // State 10
        0, 0, 11, -4, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 11
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 12
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 13
        32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 14
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 15
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 16
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 17
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 18
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 19
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 20
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 21
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 22
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 23
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 24
        0, 0, 11, 0, 12, -4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 25
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 26
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 27
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 28
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 29
        0, 0, 0, -39, 0, -39, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 0, 0, 0, 0, 0, 0,
        // State 30
        0, 0, 0, 0, 0, -41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 0, 0, 0, 0, 0, 0,
        // State 31
        0, -73, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 36, 0, 0, 38, 54, 76, 78, 75, 0, 0, 37, 0, 0, 0, 7,
        // State 32
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 33
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 41, 0, 0, 0, 0, 0, 0, 0, 0, 111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 34
        0, -74, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 36, 0, 0, 38, 54, 76, 78, 75, 0, 0, 37, 0, 0, 0, 7,
        // State 35
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 36
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 116, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 37
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 38
        0, 0, 11, 0, 12, -4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 39
        0, 0, 0, -39, 0, -39, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 0, 0, 0, 0, 0, 0,
        // State 40
        0, 0, 11, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 54, 76, 78, 75, 0, 0, 0, 0, 0, 0, 0,
        // State 41
        32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 42
        32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 43
        32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 44
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -30, 0, -30, -30, -30, -30,
        // State 45
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -33, 0, -33, -33, -33, -33,
        // State 46
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -37, 0, -37, -37, -37, -37,
        // State 47
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -34, 0, -34, -34, -34, -34,
        // State 48
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -32, 0, -32, -32, -32, -32,
        // State 49
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 50
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -31, 0, -31, -31, -31, -31,
        // State 51
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 52
        0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 53
        -28, 0, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, -28, 0, 0, -28, 0, 0, 0, 0, 0, 0, -28, -28, 0, -28, -28, -28, -28,
        // State 54
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -38, 0, -38, -38, -38, -38,
        // State 55
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 56
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 57
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -55, 0, -55, -55, -55, -55,
        // State 58
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 59
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -29, 0, -29, -29, -29, -29,
        // State 60
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -48, 0, -48, -48, -48, -48,
        // State 61
        0, 0, 0, 0, 0, -27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -27, 0, 0, 0, 0, 0, 0,
        // State 62
        0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 63
        -15, 0, 0, -15, 0, -15, 16, 0, 0, 15, 0, 0, 0, -15, -15, -15, -15, -15, -15, -15, 0, -15, -15, 0, 0, -15, 0, 0, 0, 0, 0, 0, -15, 0, 0, 0, 0, 0, 0,
        // State 64
        -19, 0, -19, -19, 0, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, -19, 0, -19, -19, 0, 0, -19, 0, 0, 0, 0, 0, 0, -19, 0, 0, 0, 0, 0, 0,
        // State 65
        -57, 0, 0, -57, 0, -57, 0, 0, 0, 0, 0, 0, 0, -57, 18, 23, 22, 20, 21, 19, 0, 17, -57, 0, 0, -57, 0, 0, 0, 0, 0, 0, -57, 0, 0, 0, 0, 0, 0,
        // State 66
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 67
        -54, 0, -54, -54, 0, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, -54, 0, -54, -54, 0, 0, -54, 0, 0, 0, 0, 0, 0, -54, 0, 0, 0, 0, 0, 0,
        // State 68
        -22, 0, -22, -22, 25, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, -22, 24, -22, -22, 0, 0, -22, 0, 0, 0, 0, 0, 0, -22, 0, 0, 0, 0, 0, 0,
        // State 69
        -3, 0, 0, -3, 0, -3, -3, 0, 28, -3, 0, 26, 27, -3, -3, -3, -3, -3, -3, -3, 0, -3, -3, 0, 0, -3, 0, 0, 0, 0, 0, 0, -3, 0, 0, 0, 0, 0, 0,
        // State 70
        -17, 0, -17, -17, 0, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, -17, 0, -17, -17, 0, 0, -17, 0, 0, 0, 0, 0, 0, -17, 0, 0, 0, 0, 0, 0,
        // State 71
        -46, 0, 29, -46, 0, -46, -46, 82, -46, -46, 81, -46, -46, -46, -46, -46, -46, -46, -46, -46, 0, -46, -46, 0, 0, -46, 0, 0, 0, 0, 0, 0, -46, 0, 0, 0, 0, 0, 0,
        // State 72
        -16, 0, 0, -16, 0, -16, 0, 0, 0, 0, 0, 0, 0, -16, 0, 0, 0, 0, 0, 0, 0, 0, -16, 0, 0, -16, 0, 0, 0, 0, 0, 0, -16, 0, 0, 0, 0, 0, 0,
        // State 73
        -18, 0, -18, -18, 0, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, -18, 0, -18, -18, 0, 0, -18, 0, 0, 0, 0, 0, 0, -18, 0, 0, 0, 0, 0, 0,
        // State 74
        -8, 0, -8, -8, 0, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, -8, 0, -8, -8, 0, 0, -8, 0, 0, 0, 0, 0, 0, -8, 0, 0, 0, 0, 0, 0,
        // State 75
        -47, 0, -47, -47, 0, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, -47, 0, -47, -47, 0, 0, -47, 0, 0, 0, 0, 0, 0, -47, 0, 0, 0, 0, 0, 0,
        // State 76
        -75, 0, -75, -75, 0, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, -75, 0, -75, -75, 0, 0, -75, 0, 0, 0, 0, 0, 0, -75, 0, 0, 0, 0, 0, 0,
        // State 77
        -7, 0, -7, -7, 0, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, -7, 0, -7, -7, 0, 0, -7, 0, 0, 0, 0, 0, 0, -7, 0, 0, 0, 0, 0, 0,
        // State 78
        0, 0, 0, 0, 0, -50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 79
        0, -76, -76, 0, -76, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -76, -76, 0, 0, -76, -76, -76, -76, -76, 0, -76, -76, -76, -76, -76, -76,
        // State 80
        -53, 0, -53, -53, 0, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, -53, 0, -53, -53, 0, 0, -53, 0, 0, 0, 0, 0, 0, -53, 0, 0, 0, 0, 0, 0,
        // State 81
        -52, 0, -52, -52, 0, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, -52, 0, -52, -52, 0, 0, -52, 0, 0, 0, 0, 0, 0, -52, 0, 0, 0, 0, 0, 0,
        // State 82
        0, 0, 0, 101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 83
        0, 0, 0, 0, 0, 103, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 84
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -26, 0, -26, -26, -26, -26,
        // State 85
        -2, 0, 0, -2, 0, -2, -2, 0, 28, -2, 0, 26, 27, -2, -2, -2, -2, -2, -2, -2, 0, -2, -2, 0, 0, -2, 0, 0, 0, 0, 0, 0, -2, 0, 0, 0, 0, 0, 0,
        // State 86
        -1, 0, 0, -1, 0, -1, -1, 0, 28, -1, 0, 26, 27, -1, -1, -1, -1, -1, -1, -1, 0, -1, -1, 0, 0, -1, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0,
        // State 87
        -56, 0, 0, -56, 0, -56, 0, 0, 0, 0, 0, 0, 0, -56, 18, 23, 22, 20, 21, 19, 0, 0, -56, 0, 0, -56, 0, 0, 0, 0, 0, 0, -56, 0, 0, 0, 0, 0, 0,
        // State 88
        -9, 0, 0, -9, 0, -9, 16, 0, 0, 15, 0, 0, 0, -9, -9, -9, -9, -9, -9, -9, 0, -9, -9, 0, 0, -9, 0, 0, 0, 0, 0, 0, -9, 0, 0, 0, 0, 0, 0,
        // State 89
        -14, 0, 0, -14, 0, -14, 16, 0, 0, 15, 0, 0, 0, -14, -14, -14, -14, -14, -14, -14, 0, -14, -14, 0, 0, -14, 0, 0, 0, 0, 0, 0, -14, 0, 0, 0, 0, 0, 0,
        // State 90
        -12, 0, 0, -12, 0, -12, 16, 0, 0, 15, 0, 0, 0, -12, -12, -12, -12, -12, -12, -12, 0, -12, -12, 0, 0, -12, 0, 0, 0, 0, 0, 0, -12, 0, 0, 0, 0, 0, 0,
        // State 91
        -13, 0, 0, -13, 0, -13, 16, 0, 0, 15, 0, 0, 0, -13, -13, -13, -13, -13, -13, -13, 0, -13, -13, 0, 0, -13, 0, 0, 0, 0, 0, 0, -13, 0, 0, 0, 0, 0, 0,
        // State 92
        -11, 0, 0, -11, 0, -11, 16, 0, 0, 15, 0, 0, 0, -11, -11, -11, -11, -11, -11, -11, 0, -11, -11, 0, 0, -11, 0, 0, 0, 0, 0, 0, -11, 0, 0, 0, 0, 0, 0,
        // State 93
        -10, 0, 0, -10, 0, -10, 16, 0, 0, 15, 0, 0, 0, -10, -10, -10, -10, -10, -10, -10, 0, -10, -10, 0, 0, -10, 0, 0, 0, 0, 0, 0, -10, 0, 0, 0, 0, 0, 0,
        // State 94
        0, 0, 0, 0, 39, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 95
        0, 0, 0, 0, 0, 108, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 96
        -44, 0, 0, -44, 0, -44, -44, 0, -44, -44, 0, -44, -44, -44, -44, -44, -44, -44, -44, -44, 0, -44, -44, 0, 0, -44, 0, 0, 0, 0, 0, 0, -44, 0, 0, 0, 0, 0, 0,
        // State 97
        -45, 0, 0, -45, 0, -45, -45, 0, -45, -45, 0, -45, -45, -45, -45, -45, -45, -45, -45, -45, 0, -45, -45, 0, 0, -45, 0, 0, 0, 0, 0, 0, -45, 0, 0, 0, 0, 0, 0,
        // State 98
        -43, 0, 0, -43, 0, -43, -43, 0, -43, -43, 0, -43, -43, -43, -43, -43, -43, -43, -43, -43, 0, -43, -43, 0, 0, -43, 0, 0, 0, 0, 0, 0, -43, 0, 0, 0, 0, 0, 0,
        // State 99
        0, 0, 0, 109, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 100
        -24, 0, -24, -24, 0, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, -24, 0, -24, -24, 0, 0, -24, 0, 0, 0, 0, 0, 0, -24, 0, 0, 0, 0, 0, 0,
        // State 101
        0, 0, 0, -5, 0, -5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 102
        -23, 0, -23, -23, 0, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, -23, 0, -23, -23, 0, 0, -23, 0, 0, 0, 0, 0, 0, -23, 0, 0, 0, 0, 0, 0,
        // State 103
        0, 0, 0, 0, 0, -42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 104
        0, -69, -69, 0, -69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -69, -69, 0, 0, -69, -69, -69, -69, -69, 0, 0, -69, 0, 0, 0, -69,
        // State 105
        0, 113, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 106
        0, -60, -60, 0, -60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -60, -60, 0, 0, -60, -60, -60, -60, -60, 0, 0, -60, 0, 0, 0, -60,
        // State 107
        -21, 0, -21, -21, 0, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, -21, 0, -21, -21, 0, 0, -21, 0, 0, 0, 0, 0, 0, -21, 0, 0, 0, 0, 0, 0,
        // State 108
        -51, 0, -51, -51, 0, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, -51, 0, -51, -51, 0, 0, -51, 0, 0, 0, 0, 0, 0, -51, 0, 0, 0, 0, 0, 0,
        // State 109
        0, -61, -61, 0, -61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -61, -61, 0, 0, -61, -61, -61, -61, -61, 0, 0, -61, 0, 0, 0, -61,
        // State 110
        0, -71, -71, 0, -71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -71, -71, 0, 0, -71, -71, -71, -71, -71, 0, 0, -71, 0, 0, 0, -71,
        // State 111
        0, -70, -70, 0, -70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -70, -70, 0, 0, -70, -70, -70, -70, -70, 0, 0, -70, 0, 0, 0, -70,
        // State 112
        0, -6, -6, 0, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -6, -6, 0, -6, -6, -6, -6, -6, -6, 0, -6, -6, -6, -6, -6, -6,
        // State 113
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 114
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 115
        0, -66, -66, 0, -66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -66, -66, 0, 0, -66, -66, -66, -66, -66, 0, 0, -66, 0, 0, 0, -66,
        // State 116
        0, 0, 0, 0, 0, 122, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 117
        0, 0, 0, -40, 0, -40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 118
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 123, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 119
        0, -65, -65, 0, -65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -65, -65, 0, 0, -65, -65, -65, -65, -65, 0, 0, -65, 0, 0, 0, -65,
        // State 120
        0, -64, -64, 0, -64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -64, -64, 0, 0, -64, -64, -64, -64, -64, 0, 0, -64, 0, 0, 0, -64,
        // State 121
        -20, 0, -20, -20, 0, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, -20, 0, -20, -20, 0, 0, -20, 0, 0, 0, 0, 0, 0, -20, 0, 0, 0, 0, 0, 0,
        // State 122
        0, -72, -72, 0, -72, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -72, -72, 0, 0, -72, -72, -72, -72, -72, 0, 0, -72, 0, 0, 0, -72,
        // State 123
        0, -63, -63, 0, -63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -63, -63, 0, 44, -63, -63, -63, -63, -63, 0, 0, -63, 0, 0, 0, -63,
        // State 124
        0, -62, -62, 0, -62, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -62, -62, 0, 0, -62, -62, -62, -62, -62, 0, 0, -62, 0, 0, 0, -62,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 39 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        -58,
        // State 1
        0,
        // State 2
        -59,
        // State 3
        0,
        // State 4
        0,
        // State 5
        0,
        // State 6
        0,
        // State 7
        0,
        // State 8
        0,
        // State 9
        0,
        // State 10
        0,
        // State 11
        0,
        // State 12
        0,
        // State 13
        0,
        // State 14
        0,
        // State 15
        0,
        // State 16
        0,
        // State 17
        0,
        // State 18
        0,
        // State 19
        0,
        // State 20
        0,
        // State 21
        0,
        // State 22
        0,
        // State 23
        0,
        // State 24
        0,
        // State 25
        0,
        // State 26
        0,
        // State 27
        0,
        // State 28
        0,
        // State 29
        0,
        // State 30
        0,
        // State 31
        0,
        // State 32
        0,
        // State 33
        0,
        // State 34
        0,
        // State 35
        0,
        // State 36
        0,
        // State 37
        0,
        // State 38
        0,
        // State 39
        0,
        // State 40
        0,
        // State 41
        0,
        // State 42
        0,
        // State 43
        0,
        // State 44
        -30,
        // State 45
        -33,
        // State 46
        -37,
        // State 47
        -34,
        // State 48
        -32,
        // State 49
        -79,
        // State 50
        -31,
        // State 51
        0,
        // State 52
        0,
        // State 53
        -28,
        // State 54
        -38,
        // State 55
        0,
        // State 56
        0,
        // State 57
        -55,
        // State 58
        0,
        // State 59
        -29,
        // State 60
        -48,
        // State 61
        0,
        // State 62
        0,
        // State 63
        0,
        // State 64
        0,
        // State 65
        0,
        // State 66
        0,
        // State 67
        0,
        // State 68
        0,
        // State 69
        0,
        // State 70
        0,
        // State 71
        0,
        // State 72
        0,
        // State 73
        0,
        // State 74
        0,
        // State 75
        0,
        // State 76
        0,
        // State 77
        0,
        // State 78
        0,
        // State 79
        -76,
        // State 80
        0,
        // State 81
        0,
        // State 82
        0,
        // State 83
        0,
        // State 84
        -26,
        // State 85
        0,
        // State 86
        0,
        // State 87
        0,
        // State 88
        0,
        // State 89
        0,
        // State 90
        0,
        // State 91
        0,
        // State 92
        0,
        // State 93
        0,
        // State 94
        0,
        // State 95
        0,
        // State 96
        0,
        // State 97
        0,
        // State 98
        0,
        // State 99
        0,
        // State 100
        0,
        // State 101
        0,
        // State 102
        0,
        // State 103
        0,
        // State 104
        0,
        // State 105
        0,
        // State 106
        0,
        // State 107
        0,
        // State 108
        0,
        // State 109
        0,
        // State 110
        0,
        // State 111
        0,
        // State 112
        -6,
        // State 113
        0,
        // State 114
        0,
        // State 115
        0,
        // State 116
        0,
        // State 117
        0,
        // State 118
        0,
        // State 119
        0,
        // State 120
        0,
        // State 121
        0,
        // State 122
        0,
        // State 123
        0,
        // State 124
        0,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            0 => match state {
                17 => 88,
                18 => 89,
                19 => 90,
                20 => 91,
                21 => 92,
                22 => 93,
                _ => 63,
            },
            1 => match state {
                24 => 95,
                38 => 116,
                _ => 82,
            },
            2 => match state {
                41 => 120,
                42 => 123,
                43 => 124,
                _ => 84,
            },
            3 => 64,
            4 => match state {
                16 => 87,
                _ => 65,
            },
            5 => match state {
                31 | 34 => 33,
                32 => 39,
                37 => 41,
                8 => 66,
                11 => 83,
                28 => 99,
                35 => 113,
                36 => 114,
                40 => 118,
                _ => 29,
            },
            6 => match state {
                25 => 96,
                26 => 97,
                27 => 98,
                _ => 67,
            },
            7 => 1,
            8 => 44,
            9 => match state {
                12 => 30,
                _ => 9,
            },
            10 => match state {
                1 => 52,
                3 => 55,
                4 => 56,
                5 => 57,
                6 => 58,
                7 | 12 => 61,
                23 => 94,
                _ => 68,
            },
            11 => 45,
            12 => match state {
                2 => 54,
                _ => 46,
            },
            14 => 2,
            15 => match state {
                39 => 117,
                _ => 101,
            },
            16 => match state {
                30 => 103,
                _ => 78,
            },
            17 => match state {
                14 => 85,
                15 => 86,
                _ => 69,
            },
            18 => 70,
            19 => 47,
            20 => 62,
            21 => 71,
            22 => 48,
            23 => 72,
            24 => 49,
            25 => match state {
                34 => 111,
                _ => 104,
            },
            27 => 34,
            28 => 109,
            29 => 105,
            30 => 73,
            31 => match state {
                31 | 34 => 106,
                _ => 50,
            },
            _ => 0,
        }
    }
    #[allow(clippy::needless_raw_string_hashes)]
    const __TERMINAL: &[&str] = &[
        r###"LBrace"###,
        r###"RBrace"###,
        r###"LBracket"###,
        r###"RBracket"###,
        r###"LParen"###,
        r###"RParen"###,
        r###"Plus"###,
        r###"PlusPlus"###,
        r###"Star"###,
        r###"Minus"###,
        r###"MinusMinus"###,
        r###"Div"###,
        r###"Mod"###,
        r###"Eq"###,
        r###"EqEq"###,
        r###"Neq"###,
        r###"Lt"###,
        r###"Gt"###,
        r###"Le"###,
        r###"Ge"###,
        r###"Dot"###,
        r###"DotDot"###,
        r###"Semi"###,
        r###"StrLiteral"###,
        r###"If"###,
        r###"Then"###,
        r###"Else"###,
        r###"While"###,
        r###"IDENT"###,
        r###"NUM"###,
        r###"TRUE"###,
        r###"FALSE"###,
        r###"Comma"###,
        r###"Function"###,
        r###"Return"###,
        r###"Preprocessor"###,
        r###"Import"###,
        r###"Package"###,
        r###"Var"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = usize;
        type Error = String;
        type Token = Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Vec<Item>;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 39 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::LBrace if true => Some(0),
            Token::RBrace if true => Some(1),
            Token::LBracket if true => Some(2),
            Token::RBracket if true => Some(3),
            Token::LParen if true => Some(4),
            Token::RParen if true => Some(5),
            Token::Plus if true => Some(6),
            Token::PlusPlus if true => Some(7),
            Token::Star if true => Some(8),
            Token::Minus if true => Some(9),
            Token::MinusMinus if true => Some(10),
            Token::Div if true => Some(11),
            Token::Mod if true => Some(12),
            Token::Eq if true => Some(13),
            Token::EqEq if true => Some(14),
            Token::Neq if true => Some(15),
            Token::Lt if true => Some(16),
            Token::Gt if true => Some(17),
            Token::Le if true => Some(18),
            Token::Ge if true => Some(19),
            Token::Dot if true => Some(20),
            Token::DotDot if true => Some(21),
            Token::Semi if true => Some(22),
            Token::StrLiteral(String) if true => Some(23),
            Token::If if true => Some(24),
            Token::Then if true => Some(25),
            Token::Else if true => Some(26),
            Token::While if true => Some(27),
            Token::Ident(String) if true => Some(28),
            Token::Num(i64) if true => Some(29),
            Token::Bool(true) if true => Some(30),
            Token::Bool(false) if true => Some(31),
            Token::Comma if true => Some(32),
            Token::Function if true => Some(33),
            Token::Return if true => Some(34),
            Token::Preprocessor if true => Some(35),
            Token::Import if true => Some(36),
            Token::Package if true => Some(37),
            Token::Var if true => Some(38),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28 | 29 | 30 | 31 | 32 | 33 | 34 | 35 | 36 | 37 | 38 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 0,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 4,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 6,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 6,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 6,
                }
            }
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 6,
                }
            }
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 7,
                }
            }
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 6,
                    nonterminal_produced: 8,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 9,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            28 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 11,
                }
            }
            29 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            30 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            31 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            32 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            33 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            34 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 13,
                }
            }
            35 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 13,
                }
            }
            36 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 14,
                }
            }
            37 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 14,
                }
            }
            38 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 15,
                }
            }
            39 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 15,
                }
            }
            40 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 16,
                }
            }
            41 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 16,
                }
            }
            42 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 17,
                }
            }
            43 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 17,
                }
            }
            44 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 17,
                }
            }
            45 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 17,
                }
            }
            46 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 18,
                }
            }
            47 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 19,
                }
            }
            48 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 20,
                }
            }
            49 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 20,
                }
            }
            50 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 21,
                }
            }
            51 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 21,
                }
            }
            52 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 21,
                }
            }
            53 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            54 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 22,
                }
            }
            55 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 23,
                }
            }
            56 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 23,
                }
            }
            57 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 24,
                }
            }
            58 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 24,
                }
            }
            59 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 25,
                }
            }
            60 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 25,
                }
            }
            61 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 6,
                    nonterminal_produced: 25,
                }
            }
            62 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 25,
                }
            }
            63 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 25,
                }
            }
            64 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 25,
                }
            }
            65 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 25,
                }
            }
            66 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 26,
                }
            }
            67 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 26,
                }
            }
            68 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 27,
                }
            }
            69 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 27,
                }
            }
            70 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 28,
                }
            }
            71 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 28,
                }
            }
            72 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 29,
                }
            }
            73 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 29,
                }
            }
            74 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 30,
                }
            }
            75 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 5,
                    nonterminal_produced: 31,
                }
            }
            76 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 32,
                }
            }
            77 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 32,
                }
            }
            78 => __state_machine::SimulatedReduce::Accept,
            _ => panic!("invalid reduction index {__reduce_index}",)
        }
    }
    pub struct StartParser {
        _priv: (),
    }

    impl Default for StartParser { fn default() -> Self { Self::new() } }
    impl StartParser {
        pub fn new() -> StartParser {
            StartParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Vec<Item>, __lalrpop_util::ParseError<usize, Token, String>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&usize>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Vec<Item>,__lalrpop_util::ParseError<usize, Token, String>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            28 => {
                __reduce28(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            29 => {
                __reduce29(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            30 => {
                __reduce30(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            31 => {
                __reduce31(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            32 => {
                __reduce32(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            33 => {
                __reduce33(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            34 => {
                __reduce34(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            35 => {
                __reduce35(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            36 => {
                __reduce36(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            37 => {
                __reduce37(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            38 => {
                __reduce38(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            39 => {
                __reduce39(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            40 => {
                __reduce40(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            41 => {
                __reduce41(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            42 => {
                __reduce42(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            43 => {
                __reduce43(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            44 => {
                __reduce44(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            45 => {
                __reduce45(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            46 => {
                __reduce46(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            47 => {
                __reduce47(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            48 => {
                __reduce48(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            49 => {
                __reduce49(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            50 => {
                __reduce50(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            51 => {
                __reduce51(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            52 => {
                __reduce52(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            53 => {
                __reduce53(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            54 => {
                __reduce54(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            55 => {
                __reduce55(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            56 => {
                __reduce56(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            57 => {
                __reduce57(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            58 => {
                __reduce58(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            59 => {
                __reduce59(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            60 => {
                // Stmt = Expr, StmtTail => ActionFn(24);
                assert!(__symbols.len() >= 2);
                let __sym1 = __pop_Variant15(__symbols);
                let __sym0 = __pop_Variant1(__symbols);
                let __start = __sym0.0;
                let __end = __sym1.2;
                let __nt = match super::__action24::<>(__sym0, __sym1) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                __symbols.push((__start, __Symbol::Variant13(__nt), __end));
                (2, 25)
            }
            61 => {
                __reduce61(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            62 => {
                __reduce62(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            63 => {
                __reduce63(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            64 => {
                __reduce64(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            65 => {
                __reduce65(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            66 => {
                __reduce66(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            67 => {
                __reduce67(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            68 => {
                __reduce68(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            69 => {
                __reduce69(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            70 => {
                __reduce70(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            71 => {
                __reduce71(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            72 => {
                __reduce72(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            73 => {
                __reduce73(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            74 => {
                __reduce74(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            75 => {
                __reduce75(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            76 => {
                __reduce76(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            77 => {
                __reduce77(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            78 => {
                // __Start = Start => ActionFn(0);
                let __sym0 = __pop_Variant12(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action0::<>(__sym0);
                return Some(Ok(__nt));
            }
            _ => panic!("invalid action code {__action}")
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, (), usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Expr, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, FunctionParam, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Item, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant15<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Option<Expr>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant15(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant13<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Stmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant13(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant8<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, String, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant8(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Token, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant16<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, VarDecl, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant16(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<Expr>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant10<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<FunctionParam>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant10(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant12<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<Item>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant12(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<Stmt>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant17<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<VarDecl>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant17(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant9<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, alloc::vec::Vec<Item>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant9(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant14<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, alloc::vec::Vec<Stmt>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant14(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, bool, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant11<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, i64, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant11(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AddAndMinus = AddAndMinus, Plus, MulAndDivAndMod => ActionFn(50);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action50::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AddAndMinus = AddAndMinus, Minus, MulAndDivAndMod => ActionFn(51);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action51::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AddAndMinus = MulAndDivAndMod => ActionFn(52);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action52::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 0)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ArgList =  => ActionFn(17);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action17::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (0, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ArgList = Expr, MoreArgs => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant2(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = LBrace, Stmts, RBrace => ActionFn(21);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action21::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // BooleanLiteral = TRUE => ActionFn(38);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action38::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // BooleanLiteral = FALSE => ActionFn(39);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action39::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, EqEq, AddAndMinus => ActionFn(43);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action43::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, Neq, AddAndMinus => ActionFn(44);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action44::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, Lt, AddAndMinus => ActionFn(45);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action45::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, Gt, AddAndMinus => ActionFn(46);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action46::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, Le, AddAndMinus => ActionFn(47);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action47::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = Equality, Ge, AddAndMinus => ActionFn(48);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action48::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 4)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Equality = AddAndMinus => ActionFn(49);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action49::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 4)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Expr = RangeExpr => ActionFn(40);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action40::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 5)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = Num => ActionFn(61);
        let __sym0 = __pop_Variant11(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action61::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 6)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = StringLiteral => ActionFn(62);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action62::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 6)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = BooleanLiteral => ActionFn(63);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action63::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 6)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = Ident, Dot, Ident, LParen, ArgList, RParen => ActionFn(64);
        assert!(__symbols.len() >= 6);
        let __sym5 = __pop_Variant0(__symbols);
        let __sym4 = __pop_Variant2(__symbols);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant8(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym5.2;
        let __nt = super::__action64::<>(__sym0, __sym1, __sym2, __sym3, __sym4, __sym5);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (6, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = Ident, LParen, ArgList, RParen => ActionFn(65);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action65::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (4, 6)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = Ident => ActionFn(66);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action66::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 6)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = LParen, Expr, RParen => ActionFn(67);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action67::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 6)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Factor = LBracket, ArgList, RBracket => ActionFn(68);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant2(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action68::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 6)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // FnKw = Function => ActionFn(10);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action10::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 7)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // FunctionDef = FnKw, Ident, LParen, ParamList, RParen, Block => ActionFn(11);
        assert!(__symbols.len() >= 6);
        let __sym5 = __pop_Variant3(__symbols);
        let __sym4 = __pop_Variant0(__symbols);
        let __sym3 = __pop_Variant10(__symbols);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant8(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym5.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2, __sym3, __sym4, __sym5);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (6, 8)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // FunctionParamNode = Ident => ActionFn(16);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 9)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Ident = IDENT => ActionFn(35);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action35::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant8(__nt), __end));
        (1, 10)
    }
    fn __reduce28<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ImportDirective = Import, Ident, Semi => ActionFn(8);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant8(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant8(__nt), __end));
        (3, 11)
    }
    fn __reduce29<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode = FunctionDef => ActionFn(2);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action2::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 12)
    }
    fn __reduce30<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode = VarDecl => ActionFn(3);
        let __sym0 = __pop_Variant16(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action3::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 12)
    }
    fn __reduce31<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode = PreprocessorDirective => ActionFn(4);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action4::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 12)
    }
    fn __reduce32<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode = ImportDirective => ActionFn(5);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action5::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 12)
    }
    fn __reduce33<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode = PackageDirective => ActionFn(6);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action6::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 12)
    }
    fn __reduce34<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode* =  => ActionFn(71);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action71::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant9(__nt), __end));
        (0, 13)
    }
    fn __reduce35<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode* = ItemNode+ => ActionFn(72);
        let __sym0 = __pop_Variant9(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action72::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant9(__nt), __end));
        (1, 13)
    }
    fn __reduce36<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode+ = ItemNode => ActionFn(73);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action73::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant9(__nt), __end));
        (1, 14)
    }
    fn __reduce37<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ItemNode+ = ItemNode+, ItemNode => ActionFn(74);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant6(__symbols);
        let __sym0 = __pop_Variant9(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action74::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant9(__nt), __end));
        (2, 14)
    }
    fn __reduce38<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MoreArgs =  => ActionFn(19);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action19::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (0, 15)
    }
    fn __reduce39<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MoreArgs = Comma, Expr, MoreArgs => ActionFn(20);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action20::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (3, 15)
    }
    fn __reduce40<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MoreParams =  => ActionFn(14);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action14::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant10(__nt), __end));
        (0, 16)
    }
    fn __reduce41<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MoreParams = Comma, FunctionParamNode, MoreParams => ActionFn(15);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant10(__symbols);
        let __sym1 = __pop_Variant7(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action15::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant10(__nt), __end));
        (3, 16)
    }
    fn __reduce42<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MulAndDivAndMod = MulAndDivAndMod, Star, Factor => ActionFn(53);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action53::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 17)
    }
    fn __reduce43<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MulAndDivAndMod = MulAndDivAndMod, Div, Factor => ActionFn(54);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action54::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 17)
    }
    fn __reduce44<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MulAndDivAndMod = MulAndDivAndMod, Mod, Factor => ActionFn(55);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action55::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 17)
    }
    fn __reduce45<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // MulAndDivAndMod = Postfix => ActionFn(56);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action56::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 17)
    }
    fn __reduce46<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Num = NUM => ActionFn(36);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action36::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant11(__nt), __end));
        (1, 18)
    }
    fn __reduce47<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // PackageDirective = Package, Ident, Semi => ActionFn(9);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant8(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant8(__nt), __end));
        (3, 19)
    }
    fn __reduce48<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ParamList =  => ActionFn(12);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action12::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant10(__nt), __end));
        (0, 20)
    }
    fn __reduce49<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ParamList = FunctionParamNode, MoreParams => ActionFn(13);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant10(__symbols);
        let __sym0 = __pop_Variant7(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action13::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant10(__nt), __end));
        (2, 20)
    }
    fn __reduce50<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Postfix = Postfix, LBracket, Expr, RBracket => ActionFn(57);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action57::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (4, 21)
    }
    fn __reduce51<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Postfix = Postfix, PlusPlus => ActionFn(58);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action58::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 21)
    }
    fn __reduce52<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Postfix = Postfix, MinusMinus => ActionFn(59);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action59::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 21)
    }
    fn __reduce53<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Postfix = Factor => ActionFn(60);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action60::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 21)
    }
    fn __reduce54<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // PreprocessorDirective = Preprocessor, Ident => ActionFn(7);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant8(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action7::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant8(__nt), __end));
        (2, 22)
    }
    fn __reduce55<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // RangeExpr = Equality, DotDot, Equality => ActionFn(41);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action41::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 23)
    }
    fn __reduce56<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // RangeExpr = Equality => ActionFn(42);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action42::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 23)
    }
    fn __reduce57<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Start =  => ActionFn(77);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action77::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant12(__nt), __end));
        (0, 24)
    }
    fn __reduce58<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Start = ItemNode+ => ActionFn(78);
        let __sym0 = __pop_Variant9(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action78::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant12(__nt), __end));
        (1, 24)
    }
    fn __reduce59<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = VarDecl => ActionFn(23);
        let __sym0 = __pop_Variant16(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (1, 25)
    }
    fn __reduce61<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = If, Expr, Then, Block, Else, Block => ActionFn(25);
        assert!(__symbols.len() >= 6);
        let __sym5 = __pop_Variant3(__symbols);
        let __sym4 = __pop_Variant0(__symbols);
        let __sym3 = __pop_Variant3(__symbols);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym5.2;
        let __nt = super::__action25::<>(__sym0, __sym1, __sym2, __sym3, __sym4, __sym5);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (6, 25)
    }
    fn __reduce62<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = If, Expr, Then, Block => ActionFn(26);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant3(__symbols);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (4, 25)
    }
    fn __reduce63<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = While, Expr, Block => ActionFn(27);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action27::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (3, 25)
    }
    fn __reduce64<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = Return, Expr, Semi => ActionFn(28);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action28::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (3, 25)
    }
    fn __reduce65<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt = Return, Semi => ActionFn(29);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action29::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (2, 25)
    }
    fn __reduce66<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt* =  => ActionFn(69);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action69::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (0, 26)
    }
    fn __reduce67<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt* = Stmt+ => ActionFn(70);
        let __sym0 = __pop_Variant14(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action70::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (1, 26)
    }
    fn __reduce68<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt+ = Stmt => ActionFn(75);
        let __sym0 = __pop_Variant13(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action75::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (1, 27)
    }
    fn __reduce69<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmt+ = Stmt+, Stmt => ActionFn(76);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant13(__symbols);
        let __sym0 = __pop_Variant14(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action76::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (2, 27)
    }
    fn __reduce70<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StmtTail = Semi => ActionFn(30);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action30::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (1, 28)
    }
    fn __reduce71<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StmtTail = Eq, Expr, Semi => ActionFn(31);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action31::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (3, 28)
    }
    fn __reduce72<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmts =  => ActionFn(79);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action79::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (0, 29)
    }
    fn __reduce73<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Stmts = Stmt+ => ActionFn(80);
        let __sym0 = __pop_Variant14(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action80::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (1, 29)
    }
    fn __reduce74<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StringLiteral = StrLiteral => ActionFn(37);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action37::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant8(__nt), __end));
        (1, 30)
    }
    fn __reduce75<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // VarDecl = Var, Ident, Eq, Expr, Semi => ActionFn(34);
        assert!(__symbols.len() >= 5);
        let __sym4 = __pop_Variant0(__symbols);
        let __sym3 = __pop_Variant1(__symbols);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant8(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym4.2;
        let __nt = super::__action34::<>(__sym0, __sym1, __sym2, __sym3, __sym4);
        __symbols.push((__start, __Symbol::Variant16(__nt), __end));
        (5, 31)
    }
    fn __reduce76<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // VarDecls =  => ActionFn(32);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action32::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (0, 32)
    }
    fn __reduce77<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // VarDecls = VarDecl, VarDecls => ActionFn(33);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant17(__symbols);
        let __sym0 = __pop_Variant16(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action33::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (2, 32)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Start::StartParser;

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action0<
>(
    (_, __0, _): (usize, Vec<Item>, usize),
) -> Vec<Item>
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action1<
>(
    (_, items, _): (usize, alloc::vec::Vec<Item>, usize),
) -> Vec<Item>
{
    items
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action2<
>(
    (_, __0, _): (usize, Item, usize),
) -> Item
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action3<
>(
    (_, v, _): (usize, VarDecl, usize),
) -> Item
{
    Item::VarItem(v)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action4<
>(
    (_, p, _): (usize, String, usize),
) -> Item
{
    Item::Preprocessor(p)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action5<
>(
    (_, i, _): (usize, String, usize),
) -> Item
{
    Item::Import(i)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action6<
>(
    (_, p, _): (usize, String, usize),
) -> Item
{
    Item::Package(p)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action7<
>(
    (_, _, _): (usize, Token, usize),
    (_, id, _): (usize, String, usize),
) -> String
{
    id
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action8<
>(
    (_, _, _): (usize, Token, usize),
    (_, id, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
) -> String
{
    id
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action9<
>(
    (_, _, _): (usize, Token, usize),
    (_, id, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
) -> String
{
    id
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action10<
>(
    (_, tok, _): (usize, Token, usize),
)
{
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action11<
>(
    (_, _, _): (usize, (), usize),
    (_, name, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
    (_, params, _): (usize, Vec<FunctionParam>, usize),
    (_, _, _): (usize, Token, usize),
    (_, body, _): (usize, Vec<Stmt>, usize),
) -> Item
{
    {
        Item::FunctionItem(Function {
            ident: name,
            params,
            blk: body,
        })
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action12<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<FunctionParam>
{
    vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action13<
>(
    (_, first, _): (usize, FunctionParam, usize),
    (_, rest, _): (usize, Vec<FunctionParam>, usize),
) -> Vec<FunctionParam>
{
    {
        let mut v = vec![first];
        v.extend(rest);
        v
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action14<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<FunctionParam>
{
    vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action15<
>(
    (_, _, _): (usize, Token, usize),
    (_, p, _): (usize, FunctionParam, usize),
    (_, mut rest, _): (usize, Vec<FunctionParam>, usize),
) -> Vec<FunctionParam>
{
    {
        let mut v = vec![p];
        v.append(&mut rest);
        v
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action16<
>(
    (_, id, _): (usize, String, usize),
) -> FunctionParam
{
    FunctionParam { ident: id }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action17<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<Expr>
{
    vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action18<
>(
    (_, first, _): (usize, Expr, usize),
    (_, rest, _): (usize, Vec<Expr>, usize),
) -> Vec<Expr>
{
    {
        let mut v = vec![first];
        v.extend(rest);
        v
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action19<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<Expr>
{
    vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action20<
>(
    (_, _, _): (usize, Token, usize),
    (_, a, _): (usize, Expr, usize),
    (_, mut rest, _): (usize, Vec<Expr>, usize),
) -> Vec<Expr>
{
    {
        let mut v = vec![a];
        v.append(&mut rest);
        v
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action21<
>(
    (_, _, _): (usize, Token, usize),
    (_, stmts, _): (usize, Vec<Stmt>, usize),
    (_, _, _): (usize, Token, usize),
) -> Vec<Stmt>
{
    stmts
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action22<
>(
    (_, __0, _): (usize, alloc::vec::Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action23<
>(
    (_, v, _): (usize, VarDecl, usize),
) -> Stmt
{
    Stmt::Var(v)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action24<
>(
    (_, e, _): (usize, Expr, usize),
    (_, tail, _): (usize, Option<Expr>, usize),
) -> Result<Stmt,__lalrpop_util::ParseError<usize,Token,String>>
{
    {
        match tail {
            None => Ok(Stmt::Expr(e)),
            Some(val) => {
                if let Expr::Var(id) = e {
                    Ok(Stmt::Var(VarDecl { ident: id, expr: val }))
                } else {
                    Err(ParseError::User { error: "Invalid assignment target".to_string() })
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action25<
>(
    (_, _, _): (usize, Token, usize),
    (_, c, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, then, _): (usize, Vec<Stmt>, usize),
    (_, _, _): (usize, Token, usize),
    (_, else_blk, _): (usize, Vec<Stmt>, usize),
) -> Stmt
{
    Stmt::If {
            cond: c,
            then_blk: then,
            else_blk: Some(else_blk),
        }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action26<
>(
    (_, _, _): (usize, Token, usize),
    (_, c, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, then, _): (usize, Vec<Stmt>, usize),
) -> Stmt
{
    Stmt::If {
            cond: c,
            then_blk: then,
            else_blk: None,
        }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action27<
>(
    (_, _, _): (usize, Token, usize),
    (_, c, _): (usize, Expr, usize),
    (_, body, _): (usize, Vec<Stmt>, usize),
) -> Stmt
{
    Stmt::While {
            cond: c,
            body: body,
        }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action28<
>(
    (_, _, _): (usize, Token, usize),
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Stmt
{
    Stmt::Return(Some(e))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action29<
>(
    (_, __0, _): (usize, Token, usize),
    (_, __1, _): (usize, Token, usize),
) -> Stmt
{
    Stmt::Return(None)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action30<
>(
    (_, __0, _): (usize, Token, usize),
) -> Option<Expr>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action31<
>(
    (_, _, _): (usize, Token, usize),
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Option<Expr>
{
    Some(e)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action32<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<VarDecl>
{
    vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action33<
>(
    (_, v, _): (usize, VarDecl, usize),
    (_, mut rest, _): (usize, Vec<VarDecl>, usize),
) -> Vec<VarDecl>
{
    { rest.insert(0, v); rest }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action34<
>(
    (_, _, _): (usize, Token, usize),
    (_, id, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> VarDecl
{
    VarDecl { ident: id, expr: e }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action35<
>(
    (_, tok, _): (usize, Token, usize),
) -> String
{
    {
    match tok { Token::Ident(s) => s, _ => unreachable!() }
}
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action36<
>(
    (_, tok, _): (usize, Token, usize),
) -> i64
{
    {
    match tok { Token::Num(n) => n, _ => unreachable!() }
}
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action37<
>(
    (_, s, _): (usize, Token, usize),
) -> String
{
    {
        match s {
            Token::StrLiteral(str) => str,
            _ => unreachable!(),
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action38<
>(
    (_, __0, _): (usize, Token, usize),
) -> bool
{
    true
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action39<
>(
    (_, __0, _): (usize, Token, usize),
) -> bool
{
    false
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action40<
>(
    (_, __0, _): (usize, Expr, usize),
) -> Expr
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action41<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Range(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action42<
>(
    (_, e, _): (usize, Expr, usize),
) -> Expr
{
    e
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action43<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Eq(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action44<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Neq(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action45<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Lt(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action46<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Gt(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action47<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Le(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action48<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Ge(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action49<
>(
    (_, a, _): (usize, Expr, usize),
) -> Expr
{
    a
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action50<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Add(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action51<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Minus(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action52<
>(
    (_, m, _): (usize, Expr, usize),
) -> Expr
{
    m
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action53<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Mul(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action54<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Div(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action55<
>(
    (_, l, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, r, _): (usize, Expr, usize),
) -> Expr
{
    Expr::Mod(Box::new(l), Box::new(r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action56<
>(
    (_, p, _): (usize, Expr, usize),
) -> Expr
{
    p
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action57<
>(
    (_, base, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
    (_, index, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    Expr::Index(Box::new(base), Box::new(index))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action58<
>(
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    Expr::Increment(Box::new(e))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action59<
>(
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    Expr::Decrement(Box::new(e))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action60<
>(
    (_, f, _): (usize, Expr, usize),
) -> Expr
{
    f
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action61<
>(
    (_, n, _): (usize, i64, usize),
) -> Expr
{
    Expr::Number(n)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action62<
>(
    (_, s, _): (usize, String, usize),
) -> Expr
{
    Expr::Str(s)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action63<
>(
    (_, b, _): (usize, bool, usize),
) -> Expr
{
    Expr::Bool(b)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action64<
>(
    (_, mod_name, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
    (_, func_name, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
    (_, args, _): (usize, Vec<Expr>, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    {
        Expr::ModuleAccess(mod_name, func_name, args)
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action65<
>(
    (_, id, _): (usize, String, usize),
    (_, _, _): (usize, Token, usize),
    (_, args, _): (usize, Vec<Expr>, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    Expr::Call(id, args, None)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action66<
>(
    (_, id, _): (usize, String, usize),
) -> Expr
{
    Expr::Var(id)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action67<
>(
    (_, _, _): (usize, Token, usize),
    (_, e, _): (usize, Expr, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    e
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action68<
>(
    (_, _, _): (usize, Token, usize),
    (_, args, _): (usize, Vec<Expr>, usize),
    (_, _, _): (usize, Token, usize),
) -> Expr
{
    Expr::List(args)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action69<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> alloc::vec::Vec<Stmt>
{
    alloc::vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action70<
>(
    (_, v, _): (usize, alloc::vec::Vec<Stmt>, usize),
) -> alloc::vec::Vec<Stmt>
{
    v
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action71<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> alloc::vec::Vec<Item>
{
    alloc::vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action72<
>(
    (_, v, _): (usize, alloc::vec::Vec<Item>, usize),
) -> alloc::vec::Vec<Item>
{
    v
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action73<
>(
    (_, __0, _): (usize, Item, usize),
) -> alloc::vec::Vec<Item>
{
    alloc::vec![__0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action74<
>(
    (_, v, _): (usize, alloc::vec::Vec<Item>, usize),
    (_, e, _): (usize, Item, usize),
) -> alloc::vec::Vec<Item>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action75<
>(
    (_, __0, _): (usize, Stmt, usize),
) -> alloc::vec::Vec<Stmt>
{
    alloc::vec![__0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action76<
>(
    (_, v, _): (usize, alloc::vec::Vec<Stmt>, usize),
    (_, e, _): (usize, Stmt, usize),
) -> alloc::vec::Vec<Stmt>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action77<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<Item>
{
    let __start0 = *__lookbehind;
    let __end0 = *__lookahead;
    let __temp0 = __action71(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action1(
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action78<
>(
    __0: (usize, alloc::vec::Vec<Item>, usize),
) -> Vec<Item>
{
    let __start0 = __0.0;
    let __end0 = __0.2;
    let __temp0 = __action72(
        __0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action1(
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action79<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<Stmt>
{
    let __start0 = *__lookbehind;
    let __end0 = *__lookahead;
    let __temp0 = __action69(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action22(
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action80<
>(
    __0: (usize, alloc::vec::Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    let __start0 = __0.0;
    let __end0 = __0.2;
    let __temp0 = __action70(
        __0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action22(
        __temp0,
    )
}

#[allow(clippy::type_complexity, dead_code)]
pub trait __ToTriple<>
{
    fn to_triple(self) -> Result<(usize,Token,usize), __lalrpop_util::ParseError<usize, Token, String>>;
}

impl<> __ToTriple<> for (usize, Token, usize)
{
    fn to_triple(self) -> Result<(usize,Token,usize), __lalrpop_util::ParseError<usize, Token, String>> {
        Ok(self)
    }
}
impl<> __ToTriple<> for Result<(usize, Token, usize), String>
{
    fn to_triple(self) -> Result<(usize,Token,usize), __lalrpop_util::ParseError<usize, Token, String>> {
        self.map_err(|error| __lalrpop_util::ParseError::User { error })
    }
}
