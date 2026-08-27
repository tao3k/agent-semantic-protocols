use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let resolved =
        agent_semantic_schema_manager::build_support::provider_registry::resolve_provider_register(
            manifest_dir.join("../.."),
        )
        .expect("resolve provider register");
    for path in resolved.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
