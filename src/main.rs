use core::slice;

mod ast;
mod grammar;
mod sema_builder;
mod lexer;

fn main() {
    let input = "test{a=1;b=2*3;} test2{c=4;}";
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
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => {
            let sigs = sema_builder::collect_signatures(&items);
            for sig in &sigs {
                println!("Item signature: {:?}", sig);
            }
            let vardecls = sema_builder::collect_all_vardecls(&items, &sigs);
            for (item_name, vardecl) in &vardecls {
                println!("VarDecl in {}: {:?}", item_name, vardecl);
            }
        },
        Err(e) => eprintln!("Error parsing input: {:?}", e),
    }
}