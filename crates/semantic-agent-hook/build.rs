fn main() {
    let config = rust_lang_project_harness::default_rust_harness_config();
    println!("cargo:rerun-if-changed=../../languages/rust-lang-project-harness/Cargo.toml");
    println!(
        "cargo:rustc-env=SEMANTIC_AGENT_HOOK_RUST_SOURCE_ROOTS={}",
        config.source_dir_names.join(",")
    );
}
