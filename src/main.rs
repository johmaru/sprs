mod ast;
mod grammar;
mod lexer;
mod sema_builder;
mod type_helper;

fn find_entry<'a>(sigs: &'a [sema_builder::ItemSig]) -> Result<&'a sema_builder::ItemSig, String> {
    if let Some(main) = sigs.iter().find(|s| s.name == "main") {
        return Ok(main);
    }
    sigs.first().ok_or_else(|| "no functions found".to_string())
}

fn main() {
    // Example input
    let input = r#"
        fn test() {
            a = 5;
            b = 10;
        }

        fn main() {
            x = 1;
        }
    "#;

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
    let items = match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Error parsing input: {:?}", e);
            return;
        }
    };

    let sigs = sema_builder::collect_signatures(&items);
    if sigs.is_empty() {
        eprintln!("No functions found");
        return;
    }

    let entry = match find_entry(&sigs) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("Error finding entry function: {}", e);
            return;
        }
    };

    println!("Entry function: {}", entry.name);

    // items[entry.ix] is the entry function item
}
