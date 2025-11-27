//! # Simple Rust-based compiler for a toy programming language "Sprs"
//! # Overview
//! This project implements a super simple compiler for embadded and system control to optimize programming language called "Sprs" using Rust
//! The compiler is dynamic type checking and easy to use and clear for the base of the language design.
//! 
//! # Super Thanks to
//! - [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
//! - [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
//! - [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
//! - [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
//! - [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
//! - [cargo-rdme](https://github.com/orium/cargo-rdme) - For generating README from doc comments
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
//! - Basic data types:
//!  - Int
//!  - Bool
//!  - Str
//!  - List
//!  - Range
//!  - Unit
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
//! - '__list_new' for creating a new list
//! - '__list_get' for getting an element from a list by index
//! - '__list_push' for pushing an element to the end of a list
//! - '__range_new' for creating a new range
//! - '__println' for printing values to the console
//! - '__strlen' for getting the length of a string
//! - '__malloc' for allocating memory
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
//! - Operators
//! - Arithmetic: `+`, `-`, `*`, `/`
//! - Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
//! - Increment/Decrement: `++`, `--`(only for postfix)
//! - Range creation: `..`(e.g., `1..10`)
//! - indexing: `list[index]`
//! 
//! - Built-in functions
//! - `println(value)`: Print value to the console
//! examples:
//! ```
//! println(y[1]);
//! ```
//! - `list_push(list)`: Push value to the end of the list
//! 
//! - module and preprocessor
//! 
//! - `#define` for defining macros
//! Currently this language has
//! - `#define Windows` or `#define Linux` for OS detection
//! - 'pkg' for module definition
//! - 'import' for module importing



use std::path::Path;
use std::process::Command;
use std::ptr::null;

use crate::runner::debug_run;
use crate::runner::parse_run;
use inkwell::context::Context;
use inkwell::targets::InitializationConfig;
use inkwell::targets::Target;
use inkwell::targets::TargetMachine;

mod ast;
mod builtin;
mod compiler;
mod executer;
mod grammar;
mod lexer;
mod runner;
mod sema_builder;
mod type_helper;
mod runtime;
fn main() {
    let context = Context::create();
    let builder = context.create_builder();

    let mut compiler = compiler::Compiler::new(&context, builder);

    if let Err(e) = compiler.load_and_compile_module("main") {
        eprintln!("Compile Error: {}", e);
        return;
    };
    
    let module = compiler.modules.get("main").unwrap();

   module.print_to_file("output.ll").unwrap();
   println!("Generated: output.ll");

   Target::initialize_all(&InitializationConfig::default());

   let target_triple = TargetMachine::get_default_triple();
   let target = Target::from_triple(&target_triple).map_err(|e| format!("Target error: {}", e)).unwrap();

   let target_machine = target
    .create_target_machine(
        &target_triple,
        "generic",
        "",
        inkwell::OptimizationLevel::Default,
        inkwell::targets::RelocMode::PIC,
        inkwell::targets::CodeModel::Default
    ).unwrap();

    module.set_data_layout(&target_machine.get_target_data().get_data_layout());
    module.set_triple(&target_triple);

    let obj_path = Path::new("output.o");
    target_machine
        .write_to_file(module, inkwell::targets::FileType::Object, obj_path)
        .map_err(|e| format!("Failed to write object file: {}", e))
        .unwrap();
    println!("Generated: output.o");

    println!("Compile runtime...");
    let status_runtime = Command::new("rustc")
        .args(&["src/runtime.rs", "--crate-type", "staticlib", "-o", "libruntime.a"])
        .status()
        .expect("Failed to compile runtime");

    if !status_runtime.success() {
        eprintln!("Failed to compile runtime");
        return;
    }

    println!("Linking...");
    let status_link = Command::new("clang")
        .args(&[
            "output.o",
            "libruntime.a",
            "-o", "main_exec",
            "-lm",
            "-ldl",
            "-lpthread",
        ])
        .status()
        .expect("Failed to link");

    if status_link.success() {
        println!("Successfully created executable: ./main_exec");
        
        println!("--- Running ---");
        let _ = Command::new("./main_exec").status();
    } else {
        eprintln!("Failed to link executable");
        return;
    }



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
