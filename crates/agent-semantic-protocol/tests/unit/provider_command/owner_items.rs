use super::support;

#[test]
fn search_owner_items_phrase_hit_attributes_to_parser_item() {
    let root = support::temp_project_root("search-owner-items-phrase-attribution");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn agent_session_artifact_activity() {\n    let heartbeat = true;\n}\n",
    )
    .expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .args([
            "rust",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "heartbeat",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "structuralSelector=rust://src/lib.rs#item/function/agent_session_artifact_activity"
        ),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("reason=owner-local-source-attribution"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("item=0"), "stdout={stdout}");

    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn query_owner_phrase_hit_attributes_to_parser_item() {
    let root = support::temp_project_root("query-owner-phrase-attribution");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn agent_session_artifact_activity() {\n    let heartbeat = true;\n}\n",
    )
    .expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs",
            "--query",
            "heartbeat",
            "--workspace",
            ".",
            "--names-only",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "structuralSelector=rust://src/lib.rs#item/function/agent_session_artifact_activity"
        ),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("status=hit") && stdout.contains("match=exact"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("item=0"), "stdout={stdout}");

    std::fs::remove_dir_all(root).expect("remove temp root");
}
