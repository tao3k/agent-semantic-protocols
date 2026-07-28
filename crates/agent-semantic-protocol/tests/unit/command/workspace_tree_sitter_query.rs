use super::{
    INCREMENTAL_OWNER_BUDGET, WorkspaceTreeSitterRequest, join_capture_projections,
    registered_source_path,
};

fn complete_owner(
    selector: &str,
    signature: &str,
    source_size: usize,
) -> agent_semantic_client_db::ProviderSelectorProjectionV1 {
    agent_semantic_client_db::ProviderSelectorProjectionV1 {
        structural_selector: selector.to_string(),
        capture_name: "item".to_string(),
        signature: signature.to_string(),
        item_kind: "module".to_string(),
        item_name: "root".to_string(),
        source_byte_start: 0,
        source_byte_end: source_size as u64,
    }
}

#[test]
fn workspace_search_request_requires_query_source_without_selector() {
    let request = WorkspaceTreeSitterRequest::parse(&[
        "search".to_string(),
        "--treesitter-query".to_string(),
        "(string_literal) @value".to_string(),
        "--workspace".to_string(),
        ".".to_string(),
    ])
    .expect("parse workspace query")
    .expect("workspace query request");
    assert_eq!(request.query_source, "(string_literal) @value");
    assert!(!request.json);

    let exact = WorkspaceTreeSitterRequest::parse(&[
        "query".to_string(),
        "--treesitter-query".to_string(),
        "(function_item) @function".to_string(),
        "--selector".to_string(),
        "rust://src/lib.rs#item/function/run".to_string(),
    ])
    .expect("parse exact query");
    assert!(exact.is_none());

    let migration_error = WorkspaceTreeSitterRequest::parse(&[
        "query".to_string(),
        "--treesitter-query".to_string(),
        "(function_item) @function".to_string(),
    ])
    .expect_err("workspace discovery must move to search");
    assert!(migration_error.contains("search-owned"));
}

#[test]
fn query_captures_join_canonical_selector_signature_and_byte_spans() {
    let language = agent_semantic_tree_sitter::registered_language_grammar("rust".into())
        .expect("Rust grammar");
    let query = agent_semantic_tree_sitter::compile_native_query_source(
        &language,
        "[(function_item name: (identifier) @declaration.name) (struct_item name: (type_identifier) @declaration.name)]",
    )
    .expect("compile query");
    let source = "pub fn run() {}\npub struct Record;\n";
    let captures = join_capture_projections(
        &language,
        &query,
        source,
        "src/lib.rs",
        &[complete_owner(
            "rust://src/lib.rs#item/module/root",
            "module root",
            source.len(),
        )],
    )
    .expect("join captures");

    assert_eq!(captures.len(), 2);
    assert!(captures.iter().all(|capture| {
        capture.capture_name == "declaration.name"
            && capture.structural_selector == "rust://src/lib.rs#item/module/root"
            && capture.signature == "module root"
            && capture.source_byte_start < capture.source_byte_end
            && capture.item_source_byte_start <= capture.source_byte_start
            && capture.source_byte_end <= capture.item_source_byte_end
    }));
    assert!(registered_source_path("src/lib.rs", &["rs".to_string()]));
    assert!(!registered_source_path("src/lib.py", &["rs".to_string()]));
}

#[test]
fn query_capture_without_complete_owner_item_fails_closed() {
    let language = agent_semantic_tree_sitter::registered_language_grammar("rust".into())
        .expect("Rust grammar");
    let query = agent_semantic_tree_sitter::compile_native_query_source(
        &language,
        r#"((string_literal) @value (#match? @value "asp install plugin --codex"))"#,
    )
    .expect("compile query");
    let source = r#"pub const INSTALL: &str = "asp install plugin --codex";"#;

    let error = join_capture_projections(&language, &query, source, "src/lib.rs", &[])
        .expect_err("missing complete-owner projection must fail");
    assert!(error.contains("no containing complete-owner item"));
}

#[test]
fn route_source_has_no_legacy_snapshot_blob_or_fallback_path() {
    let route = include_str!("../../../src/command/workspace_tree_sitter_query.rs");
    for forbidden in [
        "current_workspace_search_source_index_snapshot",
        "current_source_index_snapshot",
        "ClientDbSourceIndexSourceBlobs",
        "source_blobs",
        "collect_workspace_captures",
        "legacy",
        "fallback",
        "rebuild_full_workspace_merkle",
        "materialize_full_workspace_cas",
    ] {
        assert!(
            !route.contains(forbidden),
            "incremental route contains forbidden legacy symbol `{forbidden}`"
        );
    }
}

#[test]
fn cold_incremental_budget_limits_provider_subprocesses_to_one_owner() {
    assert_eq!(INCREMENTAL_OWNER_BUDGET, 1);
}
