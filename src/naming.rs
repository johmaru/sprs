//! Language naming constants.
//!
//! Single source of truth for every user-visible language name string.
//! The primary source is `Cargo.toml`'s `name` field, read at compile time
//! via `env!("CARGO_PKG_NAME")`. Change that one field and every derived
//! constant (file extension, config filename, default project name,
//! internal LLVM function name, CLI display name) follows automatically.
//!
//! The only constant that must be edited by hand is [`LANG_DISPLAY_NAME`],
//! because `Cargo.toml` names are lowercase and no `const fn` can title-case
//! a string at compile time.

/// Lowercase language name. Mirrors `Cargo.toml` `name`.
/// Used for CLI usage text, version output, and as a base for derived names.
pub const LANG_NAME: &str = env!("CARGO_PKG_NAME");

/// Display name with a leading capital (e.g. "Sprs").
/// Edit this by hand when renaming — it is the only manual constant here.
pub const LANG_DISPLAY_NAME: &str = "Sprs";

/// Source file extension, e.g. ".sprs".
pub const SOURCE_EXT: &str = concat!(".", env!("CARGO_PKG_NAME"));

/// Main source filename, e.g. "main.sprs".
pub const SOURCE_FILE: &str = concat!("main", ".", env!("CARGO_PKG_NAME"));

/// Project config filename, e.g. "sprs.toml".
pub const CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), ".toml");

/// Default project name used when `sprs init` is called without `--name`,
/// e.g. "sprs_project".
pub const DEFAULT_PROJECT_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "_project");

/// Internal LLVM symbol for the user `main` function, e.g. "_sprs_main".
/// Prefixed with `_` and suffixed with `_main` to avoid clashing with the
/// C entry point `main`.
pub const INTERNAL_MAIN_FN: &str = concat!("_", env!("CARGO_PKG_NAME"), "_main");
