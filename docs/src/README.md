# Sprs Documentation

Rust-based compiler for Sprs: a language designed for embedded and system control.

This project implements a super simple compiler for a custom programming language called Sprs using Rust and LLVM via the Inkwell library. The compiler translates Sprs source code into LLVM IR, which is then compiled into machine code for execution.

The compiler uses hybrid typing: runtime values are tag-dispatched, while `>>` annotations keep static types for checking. Unannotated code stays dynamic and easy to use.

This documentation is still under development and may change in the future.

## How to read this book

- [Getting Started](getting-started.md) covers compiler commands and project initialization.
- The **Language** chapters describe syntax, types, control flow, labels, errors, buffers, and operators.
- The **Reference** chapters list [Project Config](reference/project-config.md), built-in macros, modules, memory management, and [Compiler Errors](reference/compiler-errors.md).
- [Runtime Functions](internals/runtime-functions.md) documents compiler and runtime internals, not language-level APIs.
- [Contributing](contributing.md) describes the local development environment and documentation commands.
