#![doc = include_str!("../README.md")]

use std::error::Error;

use crate::command_helper::HelpCommand;
use crate::command_helper::get_all_arguments;
use crate::command_helper::help_print;
use crate::llvm::llvm_executer;

mod command_helper;
mod front;
mod grammar;
mod llvm;
mod naming;
mod runtime;

fn main() -> Result<(), Box<dyn Error>> {
    let argv: Vec<String> = std::env::args().collect();
    let argc = argv.len();

    if argc <= 1 {
        eprintln!("Usage: {} help --all", naming::LANG_NAME);
        return Err("invalid command".into());
    }

    let command = argv[1].as_str();

    match command {
        "init" => {
            let mut proj_name: Option<&String> = None;
            let mut force = false;
            if argc > 2 {
                let mut iter = argv[2..].iter().peekable();
                while let Some(arg) = iter.next() {
                    if arg == "--name" {
                        proj_name = iter.next();
                        if proj_name.is_none() {
                            eprintln!("Usage: {} init --name <project_name>", naming::LANG_NAME);
                            return Err("missing value for --name".into());
                        }
                    } else if arg == "--force" {
                        force = true;
                    } else {
                        eprintln!(
                            "Usage: {} init --name <project_name> [--force]",
                            naming::LANG_NAME
                        );
                        return Err(format!("invalid argument for init: {}", arg).into());
                    }
                }
            }
            if proj_name.is_none() {
                println!("Initializing project without arguments.");
            }
            command_helper::init_project(proj_name.map(|s| s.as_str()), force)?;
            Ok(())
        }
        "build" | "run" | "debug" => {
            let mut dest: Option<&String> = None;
            let mut error_format: Option<crate::front::error::ErrorFormat> = None;
            if argc > 2 {
                let mut iter = argv[2..].iter();
                while let Some(arg) = iter.next() {
                    if arg == "--dest" {
                        dest = iter.next();
                        if dest.is_none() {
                            eprintln!("Usage: {} {} --dest <path>", naming::LANG_NAME, command);
                            return Err("missing value for --dest".into());
                        }
                    } else if arg == "--error-format" {
                        let fmt_str = iter.next();
                        match fmt_str {
                            Some(s) => {
                                error_format = Some(crate::front::error::ErrorFormat::from_str(s)
                                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?);
                            }
                            None => {
                                eprintln!("Usage: {} {} --error-format <json|json-pretty|human>", naming::LANG_NAME, command);
                                return Err("missing value for --error-format".into());
                            }
                        }
                    } else {
                        eprintln!("Unknown argument: {}", arg);
                        return Err(format!("invalid argument: {}", arg).into());
                    }
                }
            }
            let mode = match command {
                "build" => llvm_executer::ExecuteMode::Build,
                "run" => llvm_executer::ExecuteMode::Run,
                "debug" => llvm_executer::ExecuteMode::Debug,
                _ => unreachable!(),
            };
            llvm_executer::build_and_run(dest.map(|s| s.as_str()), mode, error_format)?;
            Ok(())
        }
        "help" => {
            let args = get_all_arguments(&argv);
            if args.is_empty() {
                help_print(HelpCommand::NoArg);
            } else if args.contains(&"--all".to_string()) {
                help_print(HelpCommand::All);
            } else {
                eprintln!("Unknown help argument. Use --all.");
                return Err("invalid help argument".into());
            }
            Ok(())
        }
        "version" => {
        println!("{} version: {}", naming::LANG_NAME, env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => {
            eprintln!("Unknown command: {}", other);
            Err(format!("unknown command: {}", other).into())
        }
    }
}
