use crate::runner::debug_run;
use crate::runner::parse_run;
use inkwell::context::Context;

mod ast;
mod builtin;
mod compiler;
mod executer;
mod grammar;
mod lexer;
mod runner;
mod sema_builder;
mod type_helper;

fn main() {
    let context = Context::create();
    let builder = context.create_builder();

    if let Err(e) = compiler.load_and_compile_module("main") {
        eprintln!("Compile Error: {}", e);
        return;
    }

    if let Some(module) = compiler.modules.get("main") {
        module.print_to_file("output.ll").unwrap();
        println!("LLVM IR written to output.ll");
    }
    // Example input
    let input = r#"
        #define Windows

        fn test() {
            a = 5 - 1;
            b = 10;
            c = "hello" + " world";
            println(c);

            # test equality
            if a == 3 then {
                return a;
            }

            if a != 3 then {
                return a++;
            } else {
                return a + 2;
            }

            return b;
        }

        fn main() {
           x = test();
           y = [];
           z = 20;
           alpha = "test";
           beta = true;
           println(x);
           vec_push!(y, z);
           vec_push!(y, alpha);
           println(y[1]);
           # println(x + alpha);

           # test calc
              result = (x + 10) * 2;
              println(result);
           # test while
              i = 0;
                while i <= 5 {
                    println(i);
                    i = i + 1;
                }
        }
    "#;

    debug_run(input);

    match parse_run(input) {
        Ok(_) => println!("Parsing and analysis completed successfully."),
        Err(e) => eprintln!("Error: {}", e),
    }
}
