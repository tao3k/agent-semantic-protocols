use super::provider_install_receipt_matches_artifact;

#[test]
fn provider_receipt_requires_content_and_metadata_identity() {
    let root = std::env::temp_dir().join(format!(
        "asp-provider-receipt-admission-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create receipt admission fixture");
    let artifact = root.join("asp-rust");
    std::fs::write(&artifact, b"asp-rust-provider").expect("write provider artifact");
    let content =
        agent_semantic_content_identity::file_content_digest_v1(&artifact).expect("content digest");
    let metadata = agent_semantic_content_identity::file_artifact_metadata_digest_v1(&artifact)
        .expect("metadata digest");
    let mut receipt = agent_semantic_runtime::ProviderInstallReceipt {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        installed_path: artifact.clone(),
        installed_entrypoint_digest: content,
        installed_entrypoint_metadata_digest: metadata,
        execution_command_digest: "blake3-256:command".to_owned(),
    };

    assert!(
        provider_install_receipt_matches_artifact(&receipt, &artifact)
            .expect("admit exact receipt")
    );
    receipt.installed_entrypoint_digest = "blake3-256:wrong-content".to_owned();
    assert!(
        !provider_install_receipt_matches_artifact(&receipt, &artifact)
            .expect("reject content drift")
    );
    receipt.installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&artifact)
            .expect("restore content digest");
    receipt.installed_entrypoint_metadata_digest = "blake3-256:wrong-metadata".to_owned();
    assert!(
        !provider_install_receipt_matches_artifact(&receipt, &artifact)
            .expect("reject metadata drift")
    );

    std::fs::remove_dir_all(root).expect("remove receipt admission fixture");
}
