use crate::runner::debug_run;
use crate::runner::parse_run;

mod ast;
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
            a = 5;
            b = 10;
            return b;
        }

        fn main() {
           x = test();
           print(x);
        }
    "#;

    debug_run(input);

    match parse_run(input) {
        Ok(_) => println!("Parsing and analysis completed successfully."),
        Err(e) => eprintln!("Error: {}", e),
    }
}
