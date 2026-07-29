use crate::naming;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub src_dir: String,
    pub out_dir: String,
    #[serde(default)]
    pub error_format: Option<String>,
}

/// Validate a project/executable name: must match `[A-Za-z0-9_-]+`.
/// Rejects path separators and traversal characters (BUG-L21 / BUG-M04).
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "name contains invalid characters (allowed: A-Z a-z 0-9 _ -): {:?}",
            name
        ));
    }
    Ok(())
}

/// Validate a subpath under the project base directory.
/// Rejects absolute paths and any `..` component so that
/// `format!("{}/{}", base, subpath)` cannot escape `base` (BUG-L21).
/// The path need not exist on disk.
pub fn validate_subpath(subpath: &str) -> Result<(), String> {
    let path = Path::new(subpath);
    if path.is_absolute() {
        return Err(format!("subpath must be relative: {:?}", subpath));
    }
    if path.components().any(|c| match c {
        std::path::Component::ParentDir => true,
        _ => false,
    }) {
        return Err(format!("subpath must not contain `..`: {:?}", subpath));
    }
    Ok(())
}

pub fn get_all_arguments(args: Vec<String>) -> Vec<String> {
    args.iter()
        .filter(|arg| arg.starts_with("--"))
        .cloned()
        .collect()
}

pub fn init_project(
    mut name: Option<&str>,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    if name.is_none() {
        name = Some(naming::DEFAULT_PROJECT_NAME);
    }

    let name = name.unwrap();
    validate_name(name)?;
    let toml_path = naming::CONFIG_FILE;
    let src_path = format!("src/{}", naming::SOURCE_FILE);

    // Protect existing files unless --force was given (BUG-M03).
    if !force {
        if Path::new(toml_path).exists() {
            return Err(format!(
                "{} already exists. Use --force to overwrite.",
                toml_path
            )
            .into());
        }
        if Path::new(&src_path).exists() {
            return Err(format!(
                "{} already exists. Use --force to overwrite.",
                src_path
            )
            .into());
        }
    }

    println!("Initializing project with name: {}", name);

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        src_dir: "src".to_string(),
        out_dir: "out".to_string(),
        error_format: None,
    };

    let toml_str = toml::to_string_pretty(&config)?;
    let mut file = File::create(toml_path)?;
    file.write_all(toml_str.as_bytes())?;
    println!("Project initialized successfully with {}", naming::CONFIG_FILE);

    std::fs::create_dir_all("src")?;

    let default_code = format!(
        "fn main() {{\n    @println(\"Hello, {}!\");\n}}\n",
        naming::LANG_DISPLAY_NAME
    );
    let mut src_file = File::create(&src_path)?;
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
