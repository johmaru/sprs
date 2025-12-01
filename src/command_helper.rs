use std::fs::File;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub src_dir: String,
    pub out_dir: String,
}

pub fn get_all_arguments(args: Vec<String>) -> Vec<String> {
    let mut all_args = Vec::new();
    let mut skip_next = false;

    for (_idx, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        if arg.starts_with("--") {
            all_args.push(arg.clone());
        }
    }

    all_args
}

pub fn init_project(mut name: Option<&str>) {

        if name.is_none() {
            name = Some("sprs_project");
        }
    
        println!("Initializing project with name: {}", name.unwrap());

        let config = ProjectConfig {
            name: name.unwrap().to_string(),
            version: "0.1.0".to_string(),
            src_dir: "src".to_string(),
            out_dir: "out".to_string(),
        };

        match toml::to_string_pretty(&config) {
            Ok(toml_str) => {
                match File::create("sprs.toml") {
                    Ok(mut file) => {
                        if let Err(e) = std::io::Write::write_all(&mut file, toml_str.as_bytes()) {
                            eprintln!("Failed to write to sprs.toml: {}", e);
                        } else {
                            println!("Project initialized successfully with sprs.toml");
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create sprs.toml: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to serialize project config: {}", e);
            }
        }

        if let Err(e) = std::fs::create_dir_all("src") {
            eprintln!("Failed to create src directory: {}", e);
            return;
        }

        match File::create("src/main.sprs") {
            Ok(mut file) => {
                let default_code =r#"fn main() {
    println("Hello, Sprs!");
}
"#;
                if let Err(e) = std::io::Write::write_all(&mut file, default_code.as_bytes()) {
                    eprintln!("Failed to write to src/main.sprs: {}", e);
                } else {
                    println!("Created src/main.sprs with default code.");
                }
            }
            Err(e) => {
                eprintln!("Failed to create src/main.sprs: {}", e);
            }
        }

    }

pub enum HelpCommand {
    All,
    NoArg,
}

pub fn help_print(help: HelpCommand) {
    match help {
        HelpCommand::All => {
            println!("Sprs Compiler Full Help:");
            println!("Usage: sprs <source_file.sprs> [options]");
            println!("Options:");
            println!("---This Section is 'Command' Section---");
            println!("  init <?args>  Initialize the project");
            println!("  build         Build the project");
            println!("  run           Run the project");
            println!("  help          Show this help message");
            println!("  version       Show compiler version");
            println!("---This Section is 'Option' Section---");
            println!("  --name <name>  Set the name of the project");
            println!("  --all           Show all available commands and options");
            println!();
            println!(
                "This is the Sprs compiler, a simple compiler for the Sprs programming language."
            );
            println!("For more information, visit the official documentation.");
        }
        HelpCommand::NoArg => {
            println!("Sprs Compiler Help:");
            println!("Usage: sprs <source_file.sprs> [options]");
            println!("Options:");
            println!("---This Section is 'Command' Section---");
            println!("  init <?args>  Initialize the project");
            println!("  help          Show this help message");
            println!("  version       Show compiler version");
            println!("---This Section is 'Option' Section---");
            println!("  --name <name>  Set the name of the project");
            println!("  --all           Show all available commands and options");
        }
    }
}
