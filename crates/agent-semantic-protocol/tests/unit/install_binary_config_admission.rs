use super::{admit_embedded_hook_config, publish_embedded_hook_config};

#[test]
fn canonical_binary_publication_materializes_its_matching_hook_contract() {
    let root = std::env::temp_dir().join(format!(
        "asp-binary-hook-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    admit_embedded_hook_config().expect("embedded Hook config is schema-valid");
    assert_eq!(
        publish_embedded_hook_config(&root).expect("publish embedded Hook config"),
        "created"
    );
    let config = std::fs::read_to_string(root.join("hooks/config.toml"))
        .expect("read published Hook config");
    assert!(config.contains(&format!(
        "contractFingerprint = \"{}\"",
        agent_semantic_config::hook_client_contract_fingerprint()
    )));
    std::fs::remove_dir_all(root).ok();
}
