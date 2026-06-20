use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn rust_owner_items_labels_async_fn_with_declaration_name() {
    let root = temp_project_root("search-owner-rust-async-fn-symbol");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("codex-rs/agent-graph-store/src")).expect("create source");
    std::fs::write(
        root.join("codex-rs/agent-graph-store/src/local.rs"),
        "async fn local_store_upserts_and_lists_direct_children_with_status_filters() {\n    let runtime_task_queue = \"runtime task queue\";\n    let graph_store_local_persistence_protocol_cloud = runtime_task_queue;\n    assert!(!graph_store_local_persistence_protocol_cloud.is_empty());\n}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "owner",
            "codex-rs/agent-graph-store/src/local.rs",
            "items",
            "--query",
            "async|task|queue|runtime|graph|store|local|persistence|protocol|cloud",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "I=item:symbol(local_store_upserts_and_lists_direct_children_with_status_filters)@codex-rs/agent-graph-store/src/local.rs:1:5!syntax"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "pattern='((function_item name: (_) @function.name) (#eq? @function.name \"local_store_upserts_and_lists_direct_children_with_status_filters\"))'"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("item:symbol(async)"), "{stdout}");
    assert!(
        !marker.exists(),
        "Rust owner-items fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
