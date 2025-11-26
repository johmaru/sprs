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
        #define Windows

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
           beta = true;
           print(x);
           vec_push!(y, z);
           vec_push!(y, alpha);
           print(y[1]);

           # test calc
              result = (x + 10) * 2;
              print(result);

           # test while
              i = 0;
                while i <= 5 {
                    print(i);
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
