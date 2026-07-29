use super::{
    asp_command, provider, state_activation_path, state_home, state_runtime_bin, temp_project_root,
    write_activation, write_echo_provider,
};
use serde_json::json;

fn source_tree_snapshot(root: &std::path::Path) -> Vec<(String, u64, u128)> {
    fn collect(
        root: &std::path::Path,
        directory: &std::path::Path,
        entries: &mut Vec<(String, u64, u128)>,
    ) {
        let mut children = std::fs::read_dir(directory)
            .expect("read source fixture directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read source fixture entries");
        children.sort_by_key(std::fs::DirEntry::file_name);
        for child in children {
            let path = child.path();
            if path.is_dir() {
                collect(root, &path, entries);
                continue;
            }
            let metadata = child.metadata().expect("read source fixture metadata");
            let modified = metadata
                .modified()
                .expect("read source fixture modified time")
                .duration_since(std::time::UNIX_EPOCH)
                .expect("source fixture modified after epoch")
                .as_nanos();
            entries.push((
                path.strip_prefix(root)
                    .expect("source fixture is below root")
                    .to_string_lossy()
                    .to_string(),
                metadata.len(),
                modified,
            ));
        }
    }

    let source_root = root.join("src");
    let mut entries = Vec::new();
    collect(&source_root, &source_root, &mut entries);
    entries
}

#[test]
fn activation_fixture_does_not_mutate_project_source_tree() {
    let root = temp_project_root("activation-does-not-mutate-source");
    std::fs::create_dir_all(root.join("src")).expect("create source root");
    std::fs::write(root.join("src/lib.rs"), "pub struct ExistingOwner;\n")
        .expect("write existing source owner");
    let before = source_tree_snapshot(&root);

    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
            provider("julia", Vec::new()),
            provider("gerbil-scheme", Vec::new()),
        ],
    );

    let after = source_tree_snapshot(&root);
    assert_eq!(
        after, before,
        "activation must not mutate provider inventory"
    );
    assert!(
        after
            .iter()
            .all(|(path, _, _)| !path.contains("asp_scope_fixture_")),
        "activation must not inject source-scope fixtures: {after:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

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
