fn main() {
    match std::env::var("CARGO_CFG_PROCMACRO2_SEMVER_EXEMPT") {
        Ok(_) => println!("cargo:rustc-cfg=feature=\"proc_macro_spans\""),
        Err(_) => {}
    }
}
