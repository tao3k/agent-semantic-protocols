#[path = "../agent-semantic-schema-manager/src/build_support/provider_registry.rs"]
mod provider_registry;

fn main() {
    let manifest =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest.join("../..");
    let resolved =
        provider_registry::resolve_provider_register(&root).expect("resolve provider register");
    for path in &resolved.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir"))
        .join("provider-register.resolved.json");
    std::fs::write(out, resolved.bytes).expect("write resolved provider register");
}
