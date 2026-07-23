use crate::naming;
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
    args.iter()
        .filter(|arg| arg.starts_with("--"))
        .cloned()
        .collect()
}

pub fn init_project(mut name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    if name.is_none() {
        name = Some(naming::DEFAULT_PROJECT_NAME);
    }

    let name = name.unwrap();
    println!("Initializing project with name: {}", name);

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        src_dir: "src".to_string(),
        out_dir: "out".to_string(),
    };

    let toml_str = toml::to_string_pretty(&config)?;
    let mut file = File::create(naming::CONFIG_FILE)?;
    file.write_all(toml_str.as_bytes())?;
    println!("Project initialized successfully with {}", naming::CONFIG_FILE);

    std::fs::create_dir_all("src")?;

    let default_code = format!(
        "fn main() {{\n    @println(\"Hello, {}!\");\n}}\n",
        naming::LANG_DISPLAY_NAME
    );
    let mut src_file = File::create(format!("src/{}", naming::SOURCE_FILE))?;
    src_file.write_all(default_code.as_bytes())?;
    println!("Created src/{} with default code.", naming::SOURCE_FILE);

    Ok(())
}

pub enum HelpCommand {
    All,
    NoArg,
}

pub fn help_print(help: HelpCommand) {
    match help {
        HelpCommand::All => {
            println!("{} Compiler Full Help:", naming::LANG_DISPLAY_NAME);
            println!("Usage: {} <source_file{}> [options]", naming::LANG_NAME, naming::SOURCE_EXT);
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
                "{} is the {} compiler, a simple compiler for the {} programming language.",
                naming::LANG_DISPLAY_NAME, naming::LANG_DISPLAY_NAME, naming::LANG_DISPLAY_NAME
            );
            println!("For more information, visit the official documentation.");
        }
        HelpCommand::NoArg => {
            println!("{} Compiler Help:", naming::LANG_DISPLAY_NAME);
            println!("Usage: {} <source_file{}> [options]", naming::LANG_NAME, naming::SOURCE_EXT);
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
