fn main() {
    println!("cargo:rerun-if-changed=src/grammar.lalrpop");

    let mut cfg = lalrpop::Configuration::new();
    cfg.set_in_dir("src");
    cfg.set_out_dir("src");
    cfg.process().expect("Failed to process grammar.lalrpop. Ensure lalrpop is configured correctly and the grammar file is valid.");
}
