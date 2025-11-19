use crate::executer;
use crate::grammar;
use crate::lexer;
use crate::sema_builder;

fn find_entry<'a>(sigs: &'a [sema_builder::ItemSig]) -> Result<&'a sema_builder::ItemSig, String> {
    if let Some(main) = sigs.iter().find(|s| s.name == "main") {
        return Ok(main);
    }
    sigs.first().ok_or_else(|| "no functions found".to_string())
}

pub fn debug_run(input: &str) -> () {
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
}

pub fn parse_run(input: &str) -> Result<(), String> {
    // Parse the input
    let mut lex = lexer::Lexer::new(input);
    let items = match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Error parsing input: {:?}", e);
            return Err(format!("Error parsing input: {:?}", e));
        }
    };

    let sigs = sema_builder::collect_signatures(&items);
    if sigs.is_empty() {
        eprintln!("No functions found");
        return Err("No functions found".to_string());
    }

    let var_table = sema_builder::build_var_table(&items, &sigs);
    for sig in &sigs {
        if let Some(vars) = var_table.get(&sig.name as &str) {
            for var in vars {
                println!("  {}: {:?}", var.decl.ident, var.ty_hint);
            }
        }
    }

    let entry = match find_entry(&sigs) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("Error finding entry function: {}", e);
            return Err(format!("Error finding entry function: {}", e));
        }
    };

    println!("Entry function: {}", entry.name);

    match executer::execute(&items, entry.ix) {
        Ok(_) => println!("Execution completed successfully."),
        Err(e) => {
            eprintln!("Error during execution: {}", e);
            return Err(format!("Error during execution: {}", e));
        }
    }

    Ok(())
}
