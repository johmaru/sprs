mod ast;
mod grammar;
mod lexer;
mod sema_builder;
mod type_helper;

fn main() {
    // Example input
    let input = "test{a=1;b=2*3;} test2{c=4;}";

    // Debug: print all tokens
    {
        let mut lex = lexer::Lexer::new(input);
        println!("-- tokens --");
        while let Some(tok) = lex.next() {
            match tok {
                Ok((s, t, e)) => println!("{s}-{e}: {:?}", t),
                Err(e) => eprintln!("Error parsing input: {:?}", e),
            }
        }
        println!("------------");
    }

    // Parse the input
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => {
            let sigs = sema_builder::collect_signatures(&items);
            for sig in &sigs {
                println!("Item signature: {:?}", sig);
            }
            let vardecls = sema_builder::build_var_table(&items, &sigs);
            for (item_name, vardecl) in &vardecls {
                for var_info in vardecl {
                    println!(
                        "In item '{}', found var decl: {:?} with type hint: {:?}",
                        item_name, var_info.decl, var_info.ty_hint
                    );
                }
            }
        }
        Err(e) => eprintln!("Error parsing input: {:?}", e),
    }
}
