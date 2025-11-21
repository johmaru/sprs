use crate::runner::debug_run;
use crate::runner::parse_run;

mod ast;
mod builtin;
mod executer;
mod grammar;
mod lexer;
mod runner;
mod sema_builder;
mod type_helper;

fn main() {
    // Example input
    let input = r#"
        fn test() {
            a = 5 - 1;
            b = 10;
            c = "hello" + " world";
            print(c);

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
           print(x);
           vec_push!(y, z);
           vec_push!(y, alpha);
           print(y);
        }
    "#;

    debug_run(input);

    match parse_run(input) {
        Ok(_) => println!("Parsing and analysis completed successfully."),
        Err(e) => eprintln!("Error: {}", e),
    }
}
