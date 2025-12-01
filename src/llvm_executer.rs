use std::{path::Path, process::Command};

use inkwell::{
    context::Context,
    targets::{InitializationConfig, Target, TargetMachine, TargetTriple},
};

use crate::compiler::{self, OS};

pub fn execute(full_path: String) {
    let context = Context::create();
    let builder = context.create_builder();

    let mut compiler = compiler::Compiler::new(&context, builder);

    let path = "main.sprs".to_string();

    if let Err(e) = compiler.load_and_compile_module("main", Some(&path)) {
        eprintln!("Compile Error: {}", e);
        return;
    };

    Target::initialize_all(&InitializationConfig::default());

    let target_triple = if compiler.target_os == compiler::OS::Unknown {
        TargetMachine::get_default_triple()
    } else if compiler.target_os == compiler::OS::Windows {
        TargetTriple::create("x86_64-pc-windows-msvc")
    } else {
        TargetTriple::create("x86_64-pc-linux-gnu")
    };
    let target = Target::from_triple(&target_triple)
        .map_err(|e| format!("Target error: {}", e))
        .unwrap();

    let target_machine = target
        .create_target_machine(
            &target_triple,
            "generic",
            "",
            inkwell::OptimizationLevel::Default,
            inkwell::targets::RelocMode::PIC,
            inkwell::targets::CodeModel::Default,
        )
        .unwrap();

    let mut object_files = Vec::new();

    for (name, module) in &compiler.modules {
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());
        module.set_triple(&target_triple);

        let filename = format!("{}.o", name);
        let obj_path = Path::new(&filename);

        target_machine
            .write_to_file(module, inkwell::targets::FileType::Object, obj_path)
            .map_err(|e| format!("Failed to write object file: {}", e))
            .unwrap();
        println!("Generated: {}", filename);
        object_files.push(filename);
    }

    println!("Compile runtime...");
    let status_runtime = Command::new("rustc")
        .args(&[
            "src/runtime.rs",
            "--crate-type",
            "staticlib",
            "-o",
            "libruntime.a",
        ])
        .status()
        .expect("Failed to compile runtime");

    if !status_runtime.success() {
        eprintln!("Failed to compile runtime");
        return;
    }

    println!("Linking...");

    if (cfg!(target_os = "windows") && compiler.target_os != OS::Windows)
        || (cfg!(target_os = "linux") && compiler.target_os == OS::Windows)
    {
        println!(
            "[Warning] Running machine and target machine differ: host = {}, target = {}. Because maybe the generated executable will not run correctly.",
            if cfg!(target_os = "windows") {
                "Windows"
            } else {
                "Linux"
            },
            match compiler.target_os {
                OS::Windows => "Windows",
                OS::Linux => "Linux",
                OS::Unknown => "Unknown",
            }
        );
    }

    let exec_filename = match compiler.target_os {
        compiler::OS::Windows => "main_exec.exe",
        _ => "main_exec",
    };

    let mut args = object_files.clone();
    args.extend(vec![
        "libruntime.a".to_string(),
        "-o".to_string(),
        exec_filename.to_string(),
        "-lm".to_string(),
        "-ldl".to_string(),
        "-lpthread".to_string(),
    ]);

    let status_link = Command::new("clang")
        .args(&args)
        .status()
        .expect("Failed to link");

    if status_link.success() {
        println!("Successfully created executable: ./{}", exec_filename);
        println!("--- Running ---");
        if compiler.target_os == OS::Linux
            || (compiler.target_os == OS::Unknown || cfg!(target_os = "linux"))
        {
            let _ = Command::new(format!("./{}", exec_filename))
                .status()
                .expect("Failed to run executable");
        }
    } else {
        println!("--- Skipped ---");
    }
}
