use std::fs;

#[tokio::test]
async fn layer_one_fan_out_is_content_bound_and_runtime_independent() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(workspace.path().join("src")).expect("src");
    fs::write(
        workspace.path().join("src/registry.rs"),
        b"impl Registry { pub fn refresh(&mut self) {} }\n",
    )
    .expect("registry source");
    fs::write(
        workspace.path().join("src/unrelated.rs"),
        b"pub fn unrelated() {}\n",
    )
    .expect("unrelated source");

    let receipt = crate::execute_local_search_playbook_acquisition(
        workspace.path(),
        &[vec!["registry".to_owned()]],
        &[vec!["Registry|refresh".to_owned()]],
        &[vec!["Registry refresh".to_owned()]],
    )
    .await
    .expect("local acquisition");

    assert_eq!(
        receipt.schema_id,
        "agent.semantic-protocols.local-search-playbook-acquisition-receipt"
    );
    assert_eq!(receipt.schema_version, "1");
    assert!(receipt.content_generation_digest.starts_with("blake3-256:"));
    assert_eq!(receipt.fd.candidate_owner_paths, ["src/registry.rs"]);
    assert_eq!(receipt.rg.candidate_owner_paths, ["src/registry.rs"]);
    assert_eq!(receipt.tantivy.candidate_owner_paths, ["src/registry.rs"]);
    assert_eq!(receipt.candidate_owner_paths, ["src/registry.rs"]);
    assert!(
        receipt.exact_selectors.is_empty(),
        "lexical axes cannot mint selectors"
    );
}

#[tokio::test]
async fn local_acquisition_rejects_workspace_escape_and_preserves_empty_axis_evidence() {
    let workspace = tempfile::tempdir().expect("workspace");
    fs::write(workspace.path().join("README.md"), b"Registry refresh\n").expect("document");

    let receipt = crate::execute_local_search_playbook_acquisition(
        workspace.path(),
        &[],
        &[vec!["not-present".to_owned()]],
        &[],
    )
    .await
    .expect("empty evidence is a valid no-match");
    assert!(receipt.candidate_owner_paths.is_empty());
    assert!(receipt.fd.branch_candidate_owner_paths.is_empty());
    assert!(receipt.tantivy.branch_candidate_owner_paths.is_empty());

    let outside = workspace.path().join("missing/../escape");
    let error = crate::execute_local_search_playbook_acquisition(
        &outside,
        &[vec![".".to_owned()]],
        &[],
        &[],
    )
    .await
    .expect_err("non-canonical workspace must fail closed");
    assert!(error.contains("canonical workspace"), "{error}");
}
