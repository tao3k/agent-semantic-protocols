fn main() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let canonical_proto = agent_semantic_schema_manager::SchemaManager::new(&workspace_root)
        .canonical_wire_artifact_path("asp-client-protocol-v1")
        .expect("resolve Schema Manager-owned ASP Client Protocol wire artifact");
    println!("cargo:rerun-if-changed={}", canonical_proto.display());
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[canonical_proto], &[workspace_root.join("schemas")])
        .expect("compile Schema Manager-owned ASP Client Protocol gRPC transport");
}
