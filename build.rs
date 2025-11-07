fn main() {
    println!("cargo:rerun-if-changed=src/grammar.lalrpop");
    
    let mut cfg = lalrpop::Configuration::new();
    cfg.set_in_dir("src");
    cfg.set_out_dir("src");
    cfg.process().expect("lalrpop processing failed");
}
