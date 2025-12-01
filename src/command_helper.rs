use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct ProjectConfig {
    name: String,
    version: String,
    src_dir: String,
    out_dir: String,
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

pub fn init_project(name: Option<&str>) {
    if let Some(project_name) = name {
        println!("Initializing project with name: {}", project_name);

        let config = ProjectConfig {
            name: project_name.to_string(),
            version: "0.1.0".to_string(),
            src_dir: "src".to_string(),
            out_dir: "out".to_string(),
        };
    } else {
        println!("Initializing project without a specific name.");
        // Here you can add the logic to create a default project
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
