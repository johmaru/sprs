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
