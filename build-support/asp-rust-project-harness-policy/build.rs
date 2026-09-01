fn main() {
    let config = asp_rust::default_asp_rust_config();
    let build_dag = asp_rust::asp_rust_workspace_build_dag_from_env(&config)
        .unwrap_or_else(|error| panic!("derive ASP Cargo workspace Build DAG: {error}"));
    let policy_catalog = std::fs::read("src/member_policy.rs")
        .expect("read ASP Rust workspace member policy catalog");
    for package in &build_dag.packages {
        println!(
            "cargo:rerun-if-changed={}",
            package.package_root.join("Cargo.toml").display()
        );
    }
    let policy_catalog_digest = format!("blake3-256:{}", blake3::hash(&policy_catalog).to_hex());
    let material = serde_json::json!({
        "schemaId": "agent.semantic-protocols.asp-rust-workspace-build-receipt",
        "schemaVersion": "1",
        "buildDagSchemaId": build_dag.schema_id,
        "buildDagSchemaVersion": build_dag.schema_version,
        "workspaceRoot": build_dag.workspace_root,
        "packageCount": build_dag.packages.len(),
        "policyCatalogDigest": policy_catalog_digest,
    });
    let encoded = serde_json::to_vec(&material).expect("serialize ASP workspace build receipt");
    let digest = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/member_policy.rs");
    println!("cargo:rustc-env=ASP_RUST_WORKSPACE_BUILD_RECEIPT_DIGEST={digest}");
    println!(
        "cargo:rustc-env=ASP_RUST_WORKSPACE_BUILD_PACKAGE_COUNT={}",
        build_dag.packages.len()
    );
    println!("cargo:rustc-env=ASP_RUST_WORKSPACE_POLICY_CATALOG_DIGEST={policy_catalog_digest}");
}
