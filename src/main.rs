//! # Rust-based compiler for 'Sprs': A language designed for embedded and system control.
//! # Overview
//! This project implements a super simple compiler for a custom programming language called 'Sprs' using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.
//! The compiler uses hybrid typing: runtime values are tag-dispatched, while `>>`
//! annotations keep static types for checking. Unannotated code stays dynamic and easy to use.
//!
//! # Super Thanks to
//! * [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
//! * [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
//! * [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
//! * [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
//! * [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
//! * [cargo-rdme](https://github.com/orium/cargo-rdme) - For generating README from doc comments
//! * [serde](https://serde.rs/) - Serialization framework for Rust
//! * [toml](https://github.com/toml-rs/toml/tree/main/crates/toml) - TOML parsing library for Rust
//!
//! # sprs Language Specification
//!
//! attention: This is still under development and may change in the future.
//!
//! ## For the developers tutorial
//! For this language development environment setup is WSL2(Ubuntu) + VSCode is recommended.
//!
//! 1. Install Rust and WSL2(Ubuntu).
//! 2. ```sudo apt update && sudo apt install -y lsb-release wget software-properties-common gnupg```
//! 3. ```wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 22 all```
//! 4. ```sudo update-alternatives --install /usr/bin/clang clang /usr/bin/clang-22 100 && sudo update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-22 100 && sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-22 100 && sudo update-alternatives --install /usr/bin/llvm-as llvm-as /usr/bin/llvm-as-22 100 && sudo update-alternatives --install /usr/bin/llc llc /usr/bin/llc-22 100```
//! 5. ```sudo apt-get install zlib1g-dev libzstd-dev && sudo apt-get install libncurses5-dev libxml2-dev```
//! 6. Clone this repository and open it in VSCode.
//! 7. Install the Rust extension for VSCode.
//! 8. Build and run the project using `cargo build` and `cargo run`
//!
//!
//! ## Language Features
//! ### **Basic data types:**
//!  * Int (i64) — annotation keyword `int` (compatible with `i64` in type checks)
//!  * Float (f64) — annotation keyword `fp` (compatible with `fp64` / `f64` in type checks)
//!  * Bool — `bool`
//!  * Str — `str`
//!  * List (dynamic array) — annotation keyword `list`
//!  * Range — `range`
//!  * Unit — `unit`
//!  * Enum
//!  * Struct
//!  * Error (catchable) — annotation keyword `err`
//!  * i8 / u8 / i16 / u16 / i32 / u32 / i64 / u64 (mainly `@cast`; also usable in `>>` annotations)
//!  * fp16 / fp32 / fp64 (mainly `@cast`; also usable in `>>` annotations)
//!
//! - Variables and assignments
//! ```sprs
//! # Comments start with a hash symbol
//! var x = 10;
//! var name = "sprs";
//! var is_valid = true;
//! var numbers = [1, 2, 3];
//!
//!
//! # Not initialized variable
//! var y;  # y is initialized to Unit type
//!
//! # Re-assignment
//!
//! var y;
//! y = 20;
//! y = "now a string"; # y is now a string
//!
//! ```
//!
//! - Functions
//! ```
//! fn add(a, b) {
//!    return a + b;
//! }
//!
//! fn main() {
//!  result = add(5, 10);
//!  @println(result);
//! }
//! ```
//!
//! if a function is not marked as 'pub', it is private function.
//! the function can call in same module.
//!
//! Parameter and return types use `>>` annotations. Unannotated parameters stay dynamic.
//! Annotated parameters are checked at call sites (arity and type). `int` and `i64` are
//! treated as the same type for checking; `fp` and `fp64` likewise.
//! ```
//! fn add(a >> int, b >> int) >> int {
//!   return a + b;
//! }
//! ```
//!
//! Fixed annotations reject incompatible reassignment. Prefix the type with `ambi`
//! (ambiguous) when the binding should start as that type but allow dynamic reassignment:
//! ```
//! fn demo(fixed >> int, flex >> ambi int) {
//!   fixed = 1;      # ok
//!   # fixed = "x";  # type error
//!   flex = 1;       # ok
//!   flex = "x";     # ok — becomes dynamic after reassignment
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
//!   | __drop | for dropping a value|
//!   | __clone | for cloning a value|
//!   | __panic | for handling panic situations|
//!
//!
//! - enum
//!
//! ```
//!pub enum Animal {
//!  Dog,
//!  Cat,
//!}
//!
//!fn main() {
//!    # test enum
//!    @println(Animal.Dog);
//!
//!    #  Will be print out from a runtime "Value[Animal.Dog]: <enum variant index 1>"
//! }
//!
//! ```
//!
//! - struct
//!
//! ```
//! pub struct Point {
//!   x >> i64,
//!   y >> i64
//! }
//!
//! fn main() {
//!  var p = @init(Point {
//!   x = 10,
//!   y = 20
//!  });
//!
//! @println(p.x); # prints 10
//! @println(p.y); # prints 20
//! }
//! ```
//!
//! - Control flow
//! ```
//! if x > 5 {
//!   @println("x is greater than 5");
//! } else {
//!  @println("x is 5 or less");
//! }
//!
//! while x < 10 {
//!  println(x);
//!  i++;
//! }
//! ```
//!
//! ###  **Operators**
//! * Arithmetic: `+`, `-`, `*`, `/`, `%`
//! * Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
//! * Increment/Decrement: `++`, `--`(only for postfix)
//! * Range creation: `..`(e.g., `1..10`)
//! * indexing: `list[index]`
//!
//! ###  **Built-in macros**
//! * `@println(value)`: Print value to the console
//! examples:
//! ```
//! @println(y[1]);
//! ```
//! * `@list_push(list, value)`: Push value to the end of the list
//! examples:
//! ```
//! @list_push(y, z);
//! ```
//!
//! * `@clone(value)`: Clone the value
//! examples:
//! ```
//! var a = "hello";
//! @println(@clone(a));
//!
//! ```
//!
//! * `@move(value)`: Move out of a `cp` binding for one use (invalidates the binding)
//! examples:
//! ```
//! cp var a = "hello";
//! @println(@move(a)); # a becomes Unit
//! ```
//!
//! * `@cast(value, type)`: Cast the value to the specified type
//! examples:
//! ```
//! var a = 100; # default is i64
//! var b = @cast(a, i8); # cast to i8
//! @println(b); # prints 100 as i8
//! ```
//!
//! **Note:** @cast macro is faster than normal int type, because it use i8 and u8 llvm type directly.
//! examples:
//! ```
//! var i = 0; # default is i64
//! while i < 5 {
//!   @println(i); ## this is too slow for embedded and system programming environment, because it use dynamic type checking.
//!  i = i + 1;
//! }
//! ```
//!
//!  but with @cast macro
//!```
//! var i = @cast(0, i8); # i is i8 type
//! while i < @cast(5, i8) {
//!  @println(i); ## this is faster for embedded system, because it use i8 llvm type directly.
//! i = i + @cast(1, i8);
//! }
//! ```
//!
//! ###  **module and preprocessor**
//!
//! * `#define` for defining macros
//! Currently this language has
//! * `#define Windows` or `#define Linux` for OS detection
//! * 'pkg' for module definition
//! * 'import' for module importing
//!
//! examples:
//! ```
//!
//! import test;
//! #define Windows
//!
//!        fn main() {
//!           # access to module function
//!           var x = test.test();
//!           var y = [];
//!           var z = 20;
//!           var alpha = "test";
//!           var beta = true;
//!           @println(x);
//!           @list_push(y, z);
//!           @list_push(y, alpha);
//!           @println(y[1]);
//!           # println(x + alpha);
//!
//!            # test calc
//!              var result = (x + 10) * 2;
//!              @println(result);
//!            # test while
//!              var i = @cast(0, i8);
//!                while i <= 5 {
//!                    @println(i);
//!                    i = i + 1;
//!                }
//!
//!            # test mod
//!              var m = 10 % 3;
//!              @println(m);
//!        }
//!
//! ```
//!
//! ```
//!
//! pkg test;
//!
//!  fn test() {
//!            var a = 5 - 1;
//!            var b = 10;
//!            var c = "hello" + " world";
//!            @println(c);
//!
//!            # test equality
//!            if a == 3 {
//!                return a;
//!            }
//!
//!            if a != 3 {
//!                return a++;
//!            } else {
//!                return a + 2;
//!            }
//!
//!            return b;
//!       }
//! ```
//!
//! ## Compiler Usage
//! To build and run a Sprs program, use the following commands:
//! ```bash
//! # To build the project
//! sprs build
//!
//! # To run the project
//! sprs run
//! ```
//!
//! ## Project Initialization
//! To initialize a new Sprs project, use the following command:
//! ```bash
//! sprs init --name <project_name>
//! ```
//! This command creates a new directory structure with a default `sprs.toml` configuration file and a sample `main.sprs` source file.
//!
//! ## Memory Management
//!
//! Sprs uses **move semantics** for heap values (`str`, `list`, `range`, `struct`, `enum`, `error`).
//! Assigning or passing one of these values transfers ownership; the old binding becomes invalid
//! (`Unit`). Integers, floats, and bools are copied instead.
//!
//! Use `@clone(x)` when you need to keep the original value after a move.
//! Use `cp var` when the same binding is read many times and writing `@clone` each time is noisy.
//! Use `@move(x)` to opt out of that sugar for one use.
//!
//! Auto-clone from `cp` applies when ownership would otherwise move: function arguments,
//! `@println` / `@list_push`, assignment RHS, `var` / `cp var` init from another variable,
//! and `return`. It does **not** rewrite every expression operand (for example `a + b`).
//!
//! **Phase 1:** `cp` is intended mainly for `str`. Other heap types still work, but each use
//! deep-copies; the compiler warns when `cp` is clearly applied to `list` / `range` / `struct` / `enum`.
//!
//! **Move on assignment:**
//! ```
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     var copy = greeting;       # ownership moves to copy; greeting is now invalid
//!     @println(copy);            # prints: Hello, Sprs!
//!     # @println(greeting);      # would print () — greeting was moved
//! }
//! ```
//!
//! **Move into a function call:**
//! ```
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     @println(greeting);        # greeting is moved into @println and becomes invalid
//!     # @println(greeting);      # would print ()
//! }
//! ```
//!
//! **Keep ownership with `@clone`:**
//! ```
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     @println(@clone(greeting)); # prints a copy; greeting stays valid
//!     @println(greeting);         # still prints: Hello, Sprs!
//! }
//! ```
//!
//! **Always-clone binding with `cp var`:**
//! ```
//! fn main() {
//!     cp var greeting = "Hello, Sprs!";
//!     @println(greeting);         # same as @println(@clone(greeting))
//!     @println(greeting);         # still valid
//!     @println(@move(greeting));  # one-shot real move; greeting becomes Unit
//! }
//! ```

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
