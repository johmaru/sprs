//! # Rust-based compiler for 'Sprs': A language designed for embedded and system control.
//! # Overview
//! This project implements a super simple compiler for a custom programming language called 'Sprs' using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.
//! The compiler is dynamic type checking and easy to use and clear for the base of the language design.
//!
//! # Super Thanks to
//! * [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
//! * [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
//! * [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
//! * [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
//! * [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
//! * [cargo-rdme](https://github.com/orium/cargo-rdme) - For generating README from doc comments
//!
//! # sprs Language Specification
//!
//! attention: This is still under development and may change in the future and currently didn't work interpreter system.
//!
//! ## For the developers tutorial
//! For this language development environment setup is WSL2(Ubuntu) + VSCode is recommended.
//!
//! 1. Install Rust and WSL2(Ubuntu).
//! 2. ```sudo apt update && sudo apt install -y lsb-release wget software-properties-common gnupg```
//! 3. ```wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 18 all```
//! 4. ```sudo update-alternatives --install /usr/bin/clang clang /usr/bin/clang-18 100 && sudo update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-18 100 && sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-18 100 && sudo update-alternatives --install /usr/bin/llvm-as llvm-as /usr/bin/llvm-as-18 100 && sudo update-alternatives --install /usr/bin/llc llc /usr/bin/llc-18 100```
//! 5. ```sudo apt-get install zlib1g-dev libzstd-dev && sudo apt-get install libncurses5-dev libxml2-dev```
//! 6. Clone this repository and open it in VSCode.
//! 7. Install the Rust extension for VSCode.
//! 8. Build and run the project using `cargo build` and `cargo run`
//!
//!
//! ## Language Features
//! ### * Basic data types: *
//!  * Int
//!  * Bool
//!  * Str
//!  * List
//!  * Range
//!  * Unit
//!
//! - Variables and assignments
//! ```sprs
//! # Comments start with a hash symbol
//! x = 10;
//! name = "sprs";
//! is_valid = true;
//! numbers = [1, 2, 3];
//! ```
//!
//! - Functions
//! ```sprs
//! fn add(a, b) {
//!    return a + b;
//! }
//!
//! fn main() {
//!  result = add(5, 10);
//!  println(result);
//! }
//! ```
//!
//! - runtime functions
//!
//!   | Function Name   | Description                          |
//!   |-----------------|--------------------------------------|
//!   | __list_new | for creating a new list|
//!   | __list_get | for getting an element from a list by index|
//!   | __list_push | for pushing an element to the end of a list|
//!   | __range_new | for creating a new range|
//!   | __println | for printing values to the console|
//!   | __strlen | for getting the length of a string|
//!   | __malloc | for allocating memory|
//!
//! - Control flow
//! ```sprs
//! if x > 5 then {
//!   println("x is greater than 5");
//! } else {
//!  println("x is 5 or less");
//! }
//!
//! while x < 10 {
//!  println(x);
//!  i++;
//! }
//! ```
//!
//! ###  * Operators *
//! * Arithmetic: `+`, `-`, `*`, `/`, `%`
//! * Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
//! * Increment/Decrement: `++`, `--`(only for postfix)
//! * Range creation: `..`(e.g., `1..10`)
//! * indexing: `list[index]`
//!
//! ###  * Built-in functions *
//! * `println(value)`: Print value to the console
//! examples:
//! ```
//! println(y[1]);
//! ```
//! * `list_push(list)`: Push value to the end of the list
//!
//! ###  * module and preprocessor *
//!
//! * `#define` for defining macros
//! Currently this language has
//! * `#define Windows` or `#define Linux` for OS detection
//! * 'pkg' for module definition
//! * 'import' for module importing
//!
//! examples:
//! ```sprs
//!
//! import test;
//! #define Windows
//!
//!        fn main() {
//!           # access to module function
//!           x = test.test();
//!           y = [];
//!           z = 20;
//!           alpha = "test";
//!           beta = true;
//!           println(x);
//!           list_push(y, z);
//!           list_push(y, alpha);
//!           println(y[1]);
//!           # println(x + alpha);
//!
//!            # test calc
//!              result = (x + 10) * 2;
//!              println(result);
//!            # test while
//!              i = 0;
//!                while i <= 5 {
//!                    println(i);
//!                    i = i + 1;
//!                }
//!
//!            # test mod
//!              m = 10 % 3;
//!              println(m);
//!        }
//!
//! ```
//!
//! ```sprs
//!
//! pkg test;
//!
//!  fn test() {
//!            a = 5 - 1;
//!            b = 10;
//!            c = "hello" + " world";
//!            println(c);
//!
//!            # test equality
//!            if a == 3 then {
//!                return a;
//!            }
//!
//!            if a != 3 then {
//!                return a++;
//!            } else {
//!                return a + 2;
//!            }
//!
//!            return b;
//!       }
//! ```
//!
use std::path::Path;
use std::process::Command;
use std::ptr::null;

use crate::command_helper::HelpCommand;
use crate::command_helper::get_all_arguments;
use crate::command_helper::help_print;
use crate::runner::debug_run;
use crate::runner::parse_run;
use inkwell::context::Context;
use inkwell::targets::InitializationConfig;
use inkwell::targets::Target;
use inkwell::targets::TargetMachine;

mod ast;
mod builtin;
mod command_helper;
mod compiler;
mod executer;
mod grammar;
mod lexer;
mod llvm_executer;
mod runner;
mod runtime;
mod sema_builder;
mod type_helper;

fn main() {
    let argv: Vec<String> = std::env::args().collect();

    let argc = argv.len();

    if argc <= 1 {
        eprintln!("Usage: sprs help --all");
        return;
    }

    if argc > 1 {
        let _path = argv[0].clone();
        let command = argv[1].clone();

        if command == "init" {
            if argc > 2 {
                let args = get_all_arguments(argv.clone());
                if args.is_empty() {
                    eprintln!("Usage: sprs init <args>");
                    return;
                } else {
                    for arg in args {
                        if arg.starts_with("--name") {
                            let proj_name = arg.split('=').nth(1).unwrap_or("default_project");
                            println!("Initializing project with name: {}", proj_name);
                        } else {
                            eprintln!("Unknown argument: {}", arg);
                        }
                    }
                }
            } else {
                println!("Initializing project without arguments.");
                // Here you can add the logic to handle the initialization without args
            }
            return;
        }

        if command == "help" {
            let args = get_all_arguments(argv.clone());

            if args.is_empty() {
                help_print(HelpCommand::NoArg);
                return;
            } else {
                if args.contains(&"--all".to_string()) {
                    help_print(HelpCommand::All);
                    return;
                }
            }
        }
        if command == "version" {
            println!("sprs version: {}", env!("CARGO_PKG_VERSION"));
            return;
        }
    };

    llvm_executer::execute(argv[0].clone());

    // interprinter
    /*
    // Example input
    let input = r#"
        #define Windows

        fn test() {
            a = 5 - 1;
            b = 10;
            c = "hello" + " world";
            println(c);

            # test equality
            if a == 3 then {
                return a;
            }

            if a != 3 then {
                return a++;
            } else {
                return a + 2;
            }

            return b;
        }

        fn main() {
           x = test();
           y = [];
           z = 20;
           alpha = "test";
           beta = true;
           println(x);
           vec_push!(y, z);
           vec_push!(y, alpha);
           println(y[1]);
           # println(x + alpha);

           # test calc
              result = (x + 10) * 2;
              println(result);
           # test while
              i = 0;
                while i <= 5 {
                    println(i);
                    i = i + 1;
                }
        }
    "#; */

    // debug_run(input);

    /*  match parse_run(input) {
        Ok(_) => println!("Parsing and analysis completed successfully."),
        Err(e) => eprintln!("Error: {}", e),
    } */
}
