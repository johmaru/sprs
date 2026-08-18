# Sprs Compiler

Rust-based compiler for Sprs: a language designed for embedded and system control.

This project implements a super simple compiler for a custom programming language called Sprs using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.
The compiler uses hybrid typing: runtime values are tag-dispatched, while `>>`
annotations keep static types for checking. Unannotated code stays dynamic and easy to use.

This project is still under development and may change in the future.

**Documentation:** <https://johmaru.github.io/sprs/>

## Quick start

```bash
sprs init --name <project_name>
sprs build
sprs run
```

## Development

See [`docs/src/contributing.md`](docs/src/contributing.md) for the WSL2/LLVM 22 setup and local documentation commands.

## Super Thanks to

* [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
* [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
* [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
* [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
* [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
* [serde](https://serde.rs/) - Serialization framework for Rust
* [toml](https://github.com/toml-rs/toml/tree/main/crates/toml) - TOML parsing library for Rust
