use std::{path::Path, process::Command};

use inkwell::{
    context::Context,
    passes::PassBuilderOptions,
    targets::{InitializationConfig, Target, TargetMachine, TargetTriple},
};

use crate::{
    command_helper::ProjectConfig,
    llvm::compiler::{self, OS},
};

const RUNTIME_SOURCE: &str = include_str!("../runtime/runtime.rs");

#[derive(PartialEq)]
pub enum ExecuteMode {
    Build,
    Run,
    Debug,
}

pub fn build_and_run(_full_path: String, mode: ExecuteMode) {
    let context = Context::create();
    let builder = context.create_builder();

    let setting_toml_content =
        std::fs::read_to_string("sprs.toml").unwrap_or_else(|_| "".to_string());

    let config: Option<ProjectConfig> = if !setting_toml_content.is_empty() {
        match toml::from_str(&setting_toml_content) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("Failed to parse sprs.toml: {}", e);
                None
            }
        }
    } else {
        None
    };

    let src_path = config
        .as_ref()
        .map(|c| c.src_dir.clone())
        .unwrap_or_else(|| "src".to_string());

    let mut compiler = compiler::Compiler::new(&context, builder, src_path.clone());

    let path = format!("{}/main.sprs", src_path);
    let proj_name = config
        .as_ref()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "sprs_project".to_string());
    let out_dir = config
        .as_ref()
        .map(|c| c.out_dir.clone())
        .unwrap_or_else(|| "build".to_string());

    if !Path::new(&out_dir).exists() {
        std::fs::create_dir_all(&out_dir).expect("Failed to create output directory");
    }

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

        // mem2reg
        let pass_options = PassBuilderOptions::create();
        let _ = module.run_passes("mem2reg", &target_machine, pass_options);

        let ll_filename = format!("{}.ll", name);
        if let Err(e) = module.print_to_file(Path::new(&ll_filename)) {
            eprintln!("Failed to write LLVM IR to {}: {}", ll_filename, e);
        }
        println!("Generated: {}", ll_filename);

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

    let runtime_src_path = format!("{}/runtime.rs", out_dir);
    if let Err(e) = std::fs::write(&runtime_src_path, RUNTIME_SOURCE) {
        eprintln!("Failed to write runtime source: {}", e);
        return;
    }

    let runtime_lib_path = format!("{}/libruntime.a", out_dir);

    let status_runtime = Command::new("rustc")
        .args(&[
            &runtime_src_path,
            "--crate-type",
            "staticlib",
            "-o",
            &runtime_lib_path,
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
        compiler::OS::Windows => {
            format!("{}.exe", proj_name)
        }
        _ => proj_name.clone(),
    };

    let mut args = object_files.clone();
    args.extend(vec![
        runtime_lib_path,
        "-o".to_string(),
        format!("{}/{}", out_dir, exec_filename),
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
        if (mode == ExecuteMode::Run) || (mode == ExecuteMode::Build && false) {
            println!("--- Running ---");
            if compiler.target_os == OS::Linux
                || (compiler.target_os == OS::Unknown || cfg!(target_os = "linux"))
            {
                let _ = Command::new(format!("./{}/{}", out_dir, exec_filename))
                    .status()
                    .expect("Failed to run executable");
            }
        }
    } else {
        println!("--- Skipped ---");
    }
}
