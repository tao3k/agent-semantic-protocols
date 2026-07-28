use super::{
    asp_command, provider, state_activation_path, state_home, state_runtime_bin, temp_project_root,
    write_activation, write_echo_provider,
};
use serde_json::json;

#[test]
fn state_home_provider_fixture_writes_lock_for_runtime_binary() {
    let root = temp_project_root("state-home-provider-lock-fixture");
    write_echo_provider(&state_runtime_bin(&root), "rs-harness", "state-home");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let _command = asp_command(&root);

    let provider_path = state_runtime_bin(&root).join("rs-harness");
    let lock_path =
        agent_semantic_runtime::provider_receipt_dir(state_home(&root)).join("rust.lock.toml");
    let lock = std::fs::read_to_string(&lock_path).expect("read provider install lock");
    assert!(
        lock.contains("schemaId = \"asp.provider-install-lock.v1\""),
        "{lock}"
    );
    assert!(
        lock.contains(&format!("installedPath = \"{}\"", provider_path.display())),
        "{lock}"
    );
    assert!(!root.join("home/.local/bin/rs-harness").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn state_home_rust_activation_regeneration_materializes_dependency_routes() {
    let root = temp_project_root("state-home-rust-activation-dependency-routes");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"activation-routes\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(root.join("src/lib.rs"), "pub struct ActivationRoutes;\n")
        .expect("write Rust source");
    write_echo_provider(&state_runtime_bin(&root), "rs-harness", "state-home-rust");

    let output = asp_command(&root)
        .args(["rust", "guide"])
        .output()
        .expect("generate State Home activation");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activation_path = state_activation_path(&root);
    let activation: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&activation_path).expect("read regenerated activation"),
    )
    .expect("activation JSON");
    let rust_provider = activation["providers"]
        .as_array()
        .expect("activation providers")
        .iter()
        .find(|entry| entry["languageId"] == "rust")
        .expect("Rust activation provider");
    assert_eq!(
        rust_provider["routes"]["dependencyTopology"]["argv"],
        json!([
            "rs-harness",
            "search",
            "dependency-topology",
            "--json",
            "--workspace",
            "{workspace}"
        ])
    );
    assert_eq!(
        rust_provider["routes"]["dependencyTopologyMetadata"]["argv"],
        json!([
            "rs-harness",
            "search",
            "dependency-topology-metadata",
            "--json",
            "--workspace",
            "{workspace}"
        ])
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[should_panic(
    expected = "State Home v1 activation fixtures cannot embed provider command prefixes"
)]
fn activation_fixture_rejects_embedded_provider_command_prefix() {
    let _ = provider("rust", vec!["rs-harness".to_string()]);
}
