# Sprs Compiler

<!-- cargo-rdme start -->

## Rust-based compiler for 'Sprs': A language designed for embedded and system control.
## Overview
This project implements a super simple compiler for a custom programming language called 'Sprs' using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.
The compiler is dynamic type checking and easy to use and clear for the base of the language design.

## Super Thanks to
* [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
* [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
* [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
* [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
* [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
* [cargo-rdme](https://github.com/orium/cargo-rdme) - For generating README from doc comments
* [serde](https://serde.rs/) - Serialization framework for Rust
* [toml](https://github.com/toml-rs/toml/tree/main/crates/toml) - TOML parsing library for Rust

## sprs Language Specification

attention: This is still under development and may change in the future and currently didn't work interpreter system.

### For the developers tutorial
For this language development environment setup is WSL2(Ubuntu) + VSCode is recommended.

1. Install Rust and WSL2(Ubuntu).
2. ```sudo apt update && sudo apt install -y lsb-release wget software-properties-common gnupg```
3. ```wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 18 all```
4. ```sudo update-alternatives --install /usr/bin/clang clang /usr/bin/clang-18 100 && sudo update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-18 100 && sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-18 100 && sudo update-alternatives --install /usr/bin/llvm-as llvm-as /usr/bin/llvm-as-18 100 && sudo update-alternatives --install /usr/bin/llc llc /usr/bin/llc-18 100```
5. ```sudo apt-get install zlib1g-dev libzstd-dev && sudo apt-get install libncurses5-dev libxml2-dev```
6. Clone this repository and open it in VSCode.
7. Install the Rust extension for VSCode.
8. Build and run the project using `cargo build` and `cargo run`


### Language Features
#### **Basic data types:**
 * Int (i64)
 * Bool
 * Str
 * List
 * Range
 * Unit
 * i8 (only for cast! macro)
 * u8 (only for cast! macro)
 * i16 (only for cast! macro)
 * u16 (only for cast! macro)
 * i32 (only for cast! macro)
 * u32 (only for cast! macro)
 * i64 (only for cast! macro)
 * u64 (only for cast! macro)

- Variables and assignments
```sprs
# Comments start with a hash symbol
var x = 10;
var name = "sprs";
var is_valid = true;
var numbers = [1, 2, 3];


# Not initialized variable
var y;  # y is initialized to Unit type

# Re-assignment

var y;
y = 20;
y = "now a string"; # y is now a string

```

- Functions
```sprs
fn add(a, b) {
   return a + b;
}

fn main() {
 result = add(5, 10);
 println!(result);
}
```

- runtime functions

  | Function Name   | Description                          |
  |-----------------|--------------------------------------|
  | __list_new | for creating a new list|
  | __list_get | for getting an element from a list by index|
  | __list_push | for pushing an element to the end of a list|
  | __range_new | for creating a new range|
  | __println | for printing values to the console|
  | __strlen | for getting the length of a string|
  | __malloc | for allocating memory|
  | __drop | for dropping a value|
  | __clone | for cloning a value|
  | __panic | for handling panic situations|

- Control flow
```sprs
if x > 5 then {
  println!("x is greater than 5");
} else {
 println!("x is 5 or less");
}

while x < 10 {
 println(x);
 i++;
}
```

####  **Operators**
* Arithmetic: `+`, `-`, `*`, `/`, `%`
* Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
* Increment/Decrement: `++`, `--`(only for postfix)
* Range creation: `..`(e.g., `1..10`)
* indexing: `list[index]`

####  **Built-in macros**
* `println!(value)`: Print value to the console
examples:
```rust
println!(y[1]);
```
* `list_push!(list, value)`: Push value to the end of the list
examples:
```rust
list_push!(y, z);
```

* `clone!(value)`: Clone the value
examples:
```rust
var a = "hello";
println!(clone!(a));

```

* `cast!(value, type)`: Cast the value to the specified type
examples:
```rust
var a = 100; # default is i64
var b = cast!(a, i8); # cast to i8
println!(b); # prints 100 as i8
```

** Note:** cast! macro is more faster then normal int type, because it use i8 and u8 llvm type directly.
examples:
```rust
var i = 0; # default is i64
while i < 5 {
  println!(i); ## this is too slow for embedded and system programming environment, because it use dynamic type checking.
 i = i + 1;
}
```

 but with cast! macro
```rust
var i = cast!(0, i8); # i is i8 type
while i < cast!(5, i8) {
 println!(i); ## this is faster for embedded system, because it use i8 llvm type directly.
i = i + cast!(1, i8);
}
```

####  **module and preprocessor**

* `#define` for defining macros
Currently this language has
* `#define Windows` or `#define Linux` for OS detection
* 'pkg' for module definition
* 'import' for module importing

examples:
```sprs

import test;
#define Windows

       fn main() {
          # access to module function
          var x = test.test();
          var y = [];
          var z = 20;
          var alpha = "test";
          var beta = true;
          println!(x);
          list_push!(y, z);
          list_push!(y, alpha);
          println!(y[1]);
          # println(x + alpha);

           # test calc
             var result = (x + 10) * 2;
             println!(result);
           # test while
             var i = cast!(0, i8);
               while i <= 5 {
                   println!(i);
                   i = i + 1;
               }

           # test mod
             var m = 10 % 3;
             println!(m);
       }

```

```sprs

pkg test;

 fn test() {
           var a = 5 - 1;
           var b = 10;
           var c = "hello" + " world";
           println!(c);

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
```

### Compiler Usage
To build and run a Sprs program, use the following commands:
```bash
# To build the project
sprs build

# To run the project
sprs run
```

### Project Initialization
To initialize a new Sprs project, use the following command:
```bash
sprs init --name <project_name>
```
This command creates a new directory structure with a default `sprs.toml` configuration file and a sample `main.sprs` source file.

### Memory Management

The Sprs has a simple runtime move system.

**Example:**
```sprs
fn main() {
   test();
}

fn test() {
  var test = "Hello, Sprs!"; # set a string to variable
  var a = test; # move the value from test to a, test is now invalid
  return println!(a); # function call with a, a is now invalid after this line
  # if you don't want to move a 'a' variable, use clone! macro
  println!(clone!(a)); # a is still valid after this line
}

```

<!-- cargo-rdme end -->
