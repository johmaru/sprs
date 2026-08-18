# Sprs Compiler

Rust-based compiler for Sprs: a language designed for embedded and system control.

This project implements a super simple compiler for a custom programming language called Sprs using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.
The compiler uses hybrid typing: runtime values are tag-dispatched, while `>>`
annotations keep static types for checking. Unannotated code stays dynamic and easy to use.

## Designed for humans and AI working together

Sprs is being designed for software development where humans and AI work on the same codebase.
The goal is not to make the language "AI-only", but to reduce ambiguity and make code easier for both humans and machines to read, generate, review, and resume working on.

The language therefore favors:

- one canonical spelling for one concept
- predictable naming and formatting rules
- less context-sensitive syntax
- explicit ownership, allocation, conversion, IPC, and other important costs
- stable, machine-readable diagnostics with useful fix suggestions
- language constructs whose meaning can be understood locally without hidden rewrites

These rules are intended to make generated code converge toward the same style as human-written code, while keeping low-level behavior visible and controllable.

This project is still under development and may change in the future.

**Documentation:** English <https://johmaru.github.io/sprs/> · 日本語 <https://johmaru.github.io/sprs/ja/>

## Quick start

```bash
sprs init --name <project_name>
sprs build
sprs run
```

## Development

See [`docs/en/src/contributing.md`](docs/en/src/contributing.md) (English) and [`docs/ja/src/contributing.md`](docs/ja/src/contributing.md) (日本語) for the WSL2/LLVM 22 setup and local documentation commands.

## Super Thanks to

* [Inkwell](https://github.com/TheDan64/inkwell) - LLVM bindings for Rust
* [logos](https://github.com/maciejhirsz/logos) - Lexer generator for Rust
* [lalrpop](https://github.com/lalrpop/lalrpop) - LR(1) parser generator for Rust
* [Rust](https://www.rust-lang.org/) - The programming language used to implement the compiler
* [Clang/LLVM](https://clang.llvm.org/) - Used for linking and generating executables
* [serde](https://serde.rs/) - Serialization framework for Rust
* [toml](https://github.com/toml-rs/toml/tree/main/crates/toml) - TOML parsing library for Rust
