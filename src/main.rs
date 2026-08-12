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
//!  * List (dynamic array) — annotation keyword `list` (also `List(T)` application form)
//!  * Range — `range`
//!  * Unit — `unit`
//!  * Enum
//!  * Struct
//!  * Buffer — fixed-size zero-initialized byte array; annotation keyword `buffer`
//!  * RawPtr — bare address from `@raw(buf)`; annotation keyword `rawptr`
//!  * Error labels (catchable) — `err` sugar for `Label(:error, any)`
//!  * Atom (immutable name) — annotation keyword `atom` (also `Atom(:name)` application form)
//!  * Label (tagged value) — annotation keyword `label` (also `Label(:name[, T])` application form)
//!  * i8 / u8 / i16 / u16 / i32 / u32 / i64 / u64 (mainly `@cast`; also usable in `>>` annotations)
//!  * fp16 / fp32 / fp64 (mainly `@cast`; also usable in `>>` annotations)
//!
//! Type *application* in annotations uses `Name(Type, …)` (for example `List(int)`,
//! `Result(int, err)`, `Label(:ok, int)`). These are compile-time forms only: they are not runtime tags.
//! Everyday code keeps the flat keywords (`list`, `err`, `atom`, `label`, `buffer`, `rawptr`). Generics /
//! type parameters (`Param`) are not user-facing yet.
//!
//! `buffer` and `rawptr` are type keywords and cannot be used as identifiers (same for `new`,
//! `destroy`, `exist`, `unsafe`, and `defer`).
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
//! ```rust
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
//! ```rust
//! fn add(a >> int, b >> int) >> int {
//!   return a + b;
//! }
//! ```
//!
//! Fixed annotations reject incompatible reassignment. Prefix the type with `ambi`
//! (ambiguous) when the binding should start as that type but allow dynamic reassignment:
//! ```rust
//! fn demo(fixed >> int, flex >> ambi int) {
//!   fixed = 1;      # ok
//!   flex = 1;       # ok
//!   flex = "x";     # ok — becomes dynamic after reassignment
//! }
//! ```
//!
//! Applied types nest and are checked by constructor name and each argument:
//! ```rust
//! fn take(xs >> List(int)) >> List(int) {
//!   return xs;
//! }
//!
//! fn parse() >> Result(int, err) {
//!   return 1;
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
//!   | __buffer_new | allocate a Buffer |
//!   | __buffer_len | Buffer length |
//!   | __buffer_get | Buffer byte read |
//!   | __buffer_set | Buffer byte write |
//!   | __buffer_exist | Buffer liveness check |
//!   | __buffer_into_raw | move Buffer bytes to a raw address |
//!   | __raw_free | free an address from __buffer_into_raw |
//!
//!
//! - enum
//!
//! ```rust
//! pub enum Animal {
//!  Dog,
//!  Cat,
//! }
//!
//! fn main() {
//!    @println(Animal.Dog);
//!
//! }
//!
//! ```
//!
//! - struct
//!
//! ```rust
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
//! ```rust
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
//! - Labels (tagged values)
//!
//! Labels are a core feature for tagging values (`Tag::Label`), not an error-only type.
//! A label always has a name plus one payload: `{:name, payload}`.
//! A bare `:name` is an immutable Atom (`Tag::Atom`) with no payload.
//!
//! ```rust
//! var success_label = :ok;              # Atom
//! var labeled_value = {:ok, 42};        # Label with payload
//!
//! var item_index = 10;
//! var dynamic_label = {:"{item_index}-item", 42};   # name becomes "10-item"
//!
//! if @label_is(dynamic_label, :"{item_index}-item") {
//!   @println(@label_payload(dynamic_label));  # 42
//!   @println(@label_name(dynamic_label));     # "10-item"
//! }
//!
//! fn wrap(value_input >> int) >> Label(int) {
//!   var item_index = value_input;
//!   return {:"{item_index}", value_input};
//! }
//! fn wrap_named(value_input >> int) >> Label(:ok, int) {
//!   return {:ok, value_input};
//! }
//! fn take(label_value >> label) >> label {
//!   return label_value;
//! }
//!
//! @attach(wrap_named(7), <:item);   # capture into a local slot
//! @println(<:item);                 # {:ok, 7}
//! ```
//!
//! Notes:
//! - Dynamic templates reject `{}`, `{expr}`, and nested braces. Use `{ident}` only.
//! - `@attach(expr, <:name)` stores a cloned value into the function-local slot
//!   `<:name`; reading `<:name` before any `@attach` is a compile error.
//! - A bare `:name` is always an Atom and never shadows an attached slot.
//! - `?` propagates only the label named `:error`; ordinary labels such as `:ok` continue on the normal path.
//!
//! - Match
//!
//! `match` is a statement for branching on Atom / Label values with static
//! patterns. Two forms:
//!
//! - **Bind** — `match <Expr> ?(var name) { case PAT => expr break; … }`.
//!   Each arm evaluates an expression, stores it into `name`, and leaves the
//!   match. The binding is visible after the match in the same block.
//! - **No bind** — `match <Expr> { case PAT => { stmts } … }`. Arms are
//!   statement blocks (same shape as `if`).
//!
//! Patterns (v1, static names only):
//! - `case :name` — match Atom or Label by name (no payload bind)
//! - `case {:name, binder}` — Label only; bind the payload to `binder`
//!   (`_` discards it)
//!
//! ```sprs
//! fn match_label_bind() >> int {
//!   match {:ok, 7} ?(var r) {
//!     case :ok => 1 break;
//!     case :error => 0 break;
//!   }
//!   return r;
//! }
//!
//! fn match_payload_bind() >> int {
//!   match {:ok, 7} ?(var r) {
//!     case {:ok, x} => x break;
//!     case :error => 0 break;
//!   }
//!   return r;
//! }
//!
//! fn match_atom_bind() >> int {
//!   match :ok ?(var r) {
//!     case :ok => 1 break;
//!     case :error => 0 break;
//!   }
//!   return r;
//! }
//!
//! fn match_no_bind_block() >> int {
//!   var flag = 0;
//!   match :error {
//!     case :ok => { flag = 100; }
//!     case :error => { flag = 1; }
//!   }
//!   return flag;
//! }
//! ```
//!
//! Notes:
//! - Unmatched scrutinees panic with `Match failed` (process exits non-zero).
//! - Dynamic name patterns such as `case :"{i}-item"` are rejected at compile
//!   time. Prefer `@label_is` with `if` for dynamic names.
//! - The bind marker is the single token `?(`; it does not collide with
//!   postfix Try `?` (`match x? { … }` still means Try-then-match).
//!
//! ### **Error labels**
//!
//! Errors are ordinary labels, not a dedicated runtime value. `err` is syntax sugar
//! for `Label(:error, any)`, and `@error(reason)` creates `{:error, reason}` with
//! exactly one argument.
//!
//! The same value can be created directly as a normal label literal. Use
//! `Label(:error, T)` when the error name and payload type should be part of the
//! function signature:
//!
//! ```sprs
//! fn make_error_label() >> Label(:error, str) {
//!   return {:error, "file not found"};
//! }
//!
//! fn main() {
//!   var error_label_value = make_error_label();
//!   @println(@is_error(error_label_value));         # true
//!   @println(@error_message(error_label_value));    # file not found
//! }
//! ```
//!
//! `err` and `@error(reason)` are shorthand for the same label convention:
//!
//! ```sprs
//! fn make_error() >> err {
//!   return @error("file not found");
//! }
//!
//! fn show_error() {
//!   var error_value = make_error();
//!   @println(@is_error(error_value));         # true
//!   @println(@error_message(error_value));    # file not found
//!   @println(@error_message(@error(:enoent))); # :enoent
//! }
//! ```
//!
//! `@error_message` returns the String payload directly when the reason is a
//! String; other payloads are rendered using the normal value formatter. The
//! removed `@error_code` macro and the legacy `Tag::Error`/`SlotData::Error` ABI
//! are no longer available. Runtime tag `9` is intentionally unused, while
//! `Tag::Label` remains `10`.
//!
//! When an error label reaches the `main` boundary without being handled, Sprs
//! prints `Uncaught error in main` and exits. A known runtime limitation is that
//! the subsequent thread-local slot cleanup may emit a TLS destruction warning:
//! the cleanup of a label payload re-enters the same thread-local slot table after
//! it has started being destroyed. This warning occurs during process termination,
//! after the uncaught-error message, and does not change the error-label result.
//!
//! ### **Buffers**
//!
//! `new(n)` allocates a zero-initialized Buffer of `n` bytes (negative → invalid handle; `0` is a valid empty buffer).
//! Bytes are Integers in `0..=255`. Index sugar `buf[i]` reads/writes like `@bufGet` / `@bufSet`.
//! Writes truncate to the low 8 bits. Out-of-bounds `@bufGet` / `buf[i]` reads return the `Unit` sentinel
//! (same convention as list indexing); out-of-bounds writes are no-ops.
//!
//! `destroy(x)` explicitly releases a heap value and marks the binding `Unit` (double `destroy` is a no-op).
//! `exist(x)` is `true` only while `x` is a live Buffer. Scope exit still auto-`__drop`s live Buffers, so
//! explicit `destroy` is optional.
//!
//! ```sprs
//! var a = new(4);
//! @bufSet(a, 0, 10);
//! a[1] = 20;
//! @println(@bufLen(a));           # 4
//! @println(a[0] + @bufGet(a, 1)); # 30
//! @println(exist(a));             # true
//! destroy(a);
//! @println(exist(a));             # false
//! ```
//!
//! ###  **Operators**
//! * Arithmetic: `+`, `-`, `*`, `/`, `%`
//! * Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
//! * Increment/Decrement: `++`, `--`(only for postfix)
//! * Range creation: `..`(e.g., `1..10`)
//! * indexing: `list[index]` / `buf[index]` (Buffer uses byte get/set)
//!
//! ###  **Built-in macros**
//! * `@println(value)`: Print value to the console
//! examples:
//! ```rust
//! @println(y[1]);
//! ```
//! * `@list_push(list, value)`: Push value to the end of the list
//! examples:
//! ```rust
//! @list_push(y, z);
//! ```
//!
//! * `@bufLen(buf)`: Buffer length as Integer (`0` for stale / non-Buffer)
//! * `@bufGet(buf, i)`: read one byte as Integer; OOB / stale → `Unit`
//! * `@bufSet(buf, i, v)`: write low 8 bits of `v` at `i`; OOB → no-op
//! examples:
//! ```rust
//! var a = new(2);
//! @bufSet(a, 0, 7);
//! @println(@bufGet(a, 0));
//! @println(@bufLen(a));
//! ```
//!
//! * `@raw(buf)`: move Buffer ownership to a RawPtr. Requires `unsafe { ... }`.
//!   Source binding becomes `Unit`; caller must `@free` the result.
//! * `@free(p)`: release a RawPtr from `@raw`. Requires `unsafe { ... }`.
//!   Null / unknown addresses are no-ops; source binding becomes `Unit`.
//! examples:
//! ```rust
//! var b = new(2);
//! unsafe {
//!   var p = @raw(b);
//!   @free(p);
//! }
//! @println(exist(b)); # false
//! ```
//!
//! * `@clone(value)`: Clone the value
//! examples:
//! ```rust
//! var a = "hello";
//! @println(@clone(a));
//!
//! ```
//!
//! * `@move(value)`: Move out of a `cp` binding for one use (invalidates the binding)
//! examples:
//! ```rust
//! cp var a = "hello";
//! @println(@move(a)); # a becomes Unit
//! ```
//!
//! * `@cast(value, type)`: Cast the value to the specified type
//! examples:
//! ```rust
//! var a = 100; # default is i64
//! var b = @cast(a, i8); # cast to i8
//! @println(b); # prints 100 as i8
//! ```
//!
//! * `@attach(expr, <:name)`: Clone `expr` into the function-local attach slot `<:name`.
//!   Read the captured value with `<:name` (not bare `:name`). Dynamic slot names are not supported.
//! ```rust
//! @attach(compute(), <:result);
//! @println(<:result);
//! ```
//!
//! * `@label_is(value, expected)`: `true` when `value` is a label whose name matches
//!   `expected` (an Atom: `:name` or `:"{ident}-…"`).
//! * `@label_payload(value)`: Clone the label payload (Unit when not a label).
//! * `@label_name(value)`: Return the label name as `str` (`""` when not a label).
//! ```rust
//! var v = {:ok, 1};
//! if @label_is(v, :ok) {
//!   @println(@label_payload(v));
//!   @println(@label_name(v));
//! }
//! ```
//!
//! **Note:** @cast macro is faster than normal int type, because it use i8 and u8 llvm type directly.
//! examples:
//! ```rust
//! var i = 0; # default is i64
//! while i < 5 {
//!   @println(i); ## this is too slow for embedded and system programming environment, because it use dynamic type checking.
//!  i = i + 1;
//! }
//! ```
//!
//!  but with @cast macro
//! ```rust
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
//! ```rust
//!
//! import test;
//! #define Windows
//!
//!        fn main() {
//!           var x = test.test();
//!           var y = [];
//!           var z = 20;
//!           var alpha = "test";
//!           var beta = true;
//!           @println(x);
//!           @list_push(y, z);
//!           @list_push(y, alpha);
//!           @println(y[1]);
//!
//!              var result = (x + 10) * 2;
//!              @println(result);
//!              var i = @cast(0, i8);
//!                while i <= 5 {
//!                    @println(i);
//!                    i = i + 1;
//!                }
//!
//!              var m = 10 % 3;
//!              @println(m);
//!        }
//!
//! ```
//!
//! ```rust
//!
//! pkg test;
//!
//!  fn test() {
//!            var a = 5 - 1;
//!            var b = 10;
//!            var c = "hello" + " world";
//!            @println(c);
//!
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
//! Sprs uses **move semantics** for heap values (`str`, `list`, `range`, `struct`, `enum`, `label`, `buffer`).
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
//! ```rust
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     var copy = greeting;       # ownership moves to copy; greeting is now invalid
//!     @println(copy);            # prints: Hello, Sprs!
//! }
//! ```
//!
//! **Move into a function call:**
//! ```rust
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     @println(greeting);        # greeting is moved into @println and becomes invalid
//! }
//! ```
//!
//! **Keep ownership with `@clone`:**
//! ```rust
//! fn main() {
//!     var greeting = "Hello, Sprs!";
//!     @println(@clone(greeting)); # prints a copy; greeting stays valid
//!     @println(greeting);         # still prints: Hello, Sprs!
//! }
//! ```
//!
//! **Always-clone binding with `cp var`:**
//! ```rust
//! fn main() {
//!     cp var greeting = "Hello, Sprs!";
//!     @println(greeting);         # same as @println(@clone(greeting))
//!     @println(greeting);         # still valid
//!     @println(@move(greeting));  # one-shot real move; greeting becomes Unit
//! }
//! ```
//!
//! ## Buffers, destroy, and exist
//!
//! Buffers participate in the same auto-drop path as other heap values: leaving a scope without
//! `destroy` still frees a live Buffer. Prefer `destroy` / `defer destroy(...)` when you need an
//! explicit lifetime cut; `exist` reports Buffer liveness only.
//!
//! ## Unsafe, RawPtr, and defer
//!
//! `@raw` / `@free` are allowed only inside `unsafe { ... }` (nesting increments a depth counter).
//! `@raw(buf)` moves the Buffer's byte allocation to a RawPtr (bare address). After `@raw`, the
//! source binding is `Unit`, so later auto-drop / `destroy` on that binding is a no-op.
//! The caller owns the address and must `@free` it. Empty / non-Buffer / stale inputs yield a null
//! RawPtr (`0`); `@free` ignores null and unknown addresses.
//!
//! `defer <expr>;` queues `expr` and runs the queue **LIFO** at scope exit, **before** automatic
//! variable drops (including on `return`).
//!
//! ```sprs
//! fn demo() {
//!   var a = new(1);
//!   defer destroy(a);   # runs at scope exit before auto-drop
//!   @bufSet(a, 0, 1);
//!
//!   var b = new(2);
//!   defer destroy(b);
//!   unsafe {
//!     var p = @raw(b);  # b becomes Unit; deferred destroy(b) is then a no-op
//!     @free(p);
//!   }
//! }
//! ```
//!

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
