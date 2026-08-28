use std::{path::Path, process::Command};

use inkwell::{
    context::Context,
    passes::PassBuilderOptions,
    targets::{InitializationConfig, Target},
};

use crate::naming;
use crate::{
    command_helper::{ProjectConfig, validate_name, validate_subpath},
    llvm::compiler::{self, OS},
};

const RUNTIME_SOURCE: &str = include_str!("../runtime/runtime.rs");

#[derive(PartialEq)]
pub enum ExecuteMode {
    Build,
    Run,
    Debug,
}

pub fn build_and_run(
    dest: Option<&str>,
    mode: ExecuteMode,
    cli_error_format: Option<crate::front::error::ErrorFormat>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::create();
    let builder = context.create_builder();

    let base = dest.unwrap_or(".");

    let toml_path = format!("{}/{}", base, naming::CONFIG_FILE);
    let setting_toml_content = std::fs::read_to_string(&toml_path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", naming::CONFIG_FILE, e);
        "".to_string()
    });

    let config: Option<ProjectConfig> = if !setting_toml_content.is_empty() {
        match toml::from_str(&setting_toml_content) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("Failed to parse {}: {}", naming::CONFIG_FILE, e);
                None
            }
        }
    } else {
        None
    };

    // Resolve error format: CLI flag > sprs.toml > default (Human).
    let error_format = cli_error_format.unwrap_or_else(|| {
        config
            .as_ref()
            .and_then(|c| c.error_format.as_ref())
            .and_then(|fmt| crate::front::error::ErrorFormat::from_str(fmt).ok())
            .unwrap_or(crate::front::error::ErrorFormat::Human)
    });

    let src_dir = config.as_ref().map(|c| c.src_dir.as_str()).unwrap_or("src");
    validate_subpath(src_dir)?;
    let src_path = format!("{}/{}", base, src_dir);

    let path = format!("{}/{}", src_path, naming::SOURCE_FILE);
    let mut compiler = compiler::Compiler::new(&context, builder, src_path);

    let proj_name = config
        .as_ref()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| naming::DEFAULT_PROJECT_NAME.to_string());
    validate_name(&proj_name)?;
    let out_dir_raw = config
        .as_ref()
        .map(|c| c.out_dir.as_str())
        .unwrap_or("build");
    validate_subpath(out_dir_raw)?;
    let out_dir = format!("{}/{}", base, out_dir_raw);

    if !Path::new(&out_dir).exists() {
        std::fs::create_dir_all(&out_dir)?;
    }

    if let Err(e) = compiler.load_and_compile_module("main", Some(&path)) {
        let source = std::fs::read_to_string(&path).unwrap_or_default();
        let rendered = crate::front::error::render(&e, error_format, &source);
        match error_format {
            crate::front::error::ErrorFormat::Json
            | crate::front::error::ErrorFormat::JsonPretty => println!("{}", rendered),
            crate::front::error::ErrorFormat::Human => eprintln!("{}", rendered),
        }
        std::process::exit(1);
    }

    Target::initialize_x86(&InitializationConfig::default());

    let target_machine = &compiler.target_machine;

    let mut object_files = Vec::new();
    let mut temp_ll_files = Vec::new();

    for (name, module) in &compiler.modules {
        compiler.apply_module_target(module);

        // mem2reg
        let pass_options = PassBuilderOptions::create();
        if let Err(e) = module.run_passes("mem2reg", target_machine, pass_options) {
            eprintln!(
                "Warning: LLVM run_passes failed for module '{}': {}",
                name, e
            );
        }

        let ll_filename = format!("{}/{}.ll", out_dir, name);
        if let Err(e) = module.print_to_file(Path::new(&ll_filename)) {
            eprintln!("Failed to write LLVM IR to {}: {}", ll_filename, e);
        }
        println!("Generated: {}", ll_filename);
        temp_ll_files.push(ll_filename);

        let filename = format!("{}/{}.o", out_dir, name);

        target_machine
            .write_to_file(
                module,
                inkwell::targets::FileType::Object,
                Path::new(&filename),
            )
            .map_err(|e| format!("Failed to write object file: {}", e))?;
        println!("Generated: {}", filename);
        object_files.push(filename);
    }

    println!("Compile runtime...");

    let runtime_src_path = format!("{}/runtime.rs", out_dir);
    std::fs::write(&runtime_src_path, RUNTIME_SOURCE)?;

    let runtime_lib_path = format!("{}/libruntime.a", out_dir);

    let status_runtime = Command::new("rustc")
        .args([
            &runtime_src_path,
            "--crate-type",
            "staticlib",
            "-o",
            &runtime_lib_path,
        ])
        .status()
        .map_err(|e| format!("Failed to invoke rustc: {}", e))?;

    if !status_runtime.success() {
        return Err("Failed to compile runtime (rustc returned non-zero status)".into());
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
        _ => proj_name,
    };
    let exec_path = Path::new(&out_dir).join(&exec_filename);

    let mut args = object_files.clone();
    args.extend(vec![
        runtime_lib_path,
        "-o".to_string(),
        exec_path.to_string_lossy().into_owned(),
        "-lm".to_string(),
        "-ldl".to_string(),
        "-lpthread".to_string(),
    ]);

    let status_link = Command::new("clang")
        .args(&args)
        .status()
        .map_err(|e| format!("Failed to invoke clang: {}", e))?;

    if status_link.success() {
        println!("Successfully created executable: {}", exec_path.display());
        if mode == ExecuteMode::Run {
            println!("--- Running ---");
            let can_run = match compiler.target_os {
                OS::Linux => cfg!(target_os = "linux"),
                OS::Windows => cfg!(target_os = "windows"),
                OS::Unknown => true,
            };
            if can_run {
                let status = Command::new(&exec_path)
                    .status()
                    .map_err(|e| format!("Failed to run executable: {}", e))?;
                if !status.success() {
                    return Err("Executable returned non-zero status".into());
                }
            } else {
                println!(
                    "[Skip] Target OS ({}) differs from host OS ({}). Skipping execution.",
                    match compiler.target_os {
                        OS::Windows => "Windows",
                        OS::Linux => "Linux",
                        OS::Unknown => "Unknown",
                    },
                    if cfg!(target_os = "windows") {
                        "Windows"
                    } else {
                        "Linux"
                    }
                );
            }
        }
    } else {
        return Err("Linker (clang) returned non-zero status".into());
    }
    // Clean up intermediate files in release builds (BUG-M10).
    if !cfg!(debug_assertions) {
        for ll in &temp_ll_files {
            let _ = std::fs::remove_file(ll);
        }
        for obj in &object_files {
            let _ = std::fs::remove_file(obj);
        }
        let _ = std::fs::remove_file(&runtime_src_path);
    }
    Ok(())
}
