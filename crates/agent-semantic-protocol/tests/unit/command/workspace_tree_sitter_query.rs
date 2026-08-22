use super::{
    TREE_SITTER_OWNER_BUDGET, WorkspaceTreeSitterRequest, join_capture_projections,
    provider_path_is_ignored, registered_source_path,
};

fn complete_owner(
    selector: &str,
    signature: &str,
    source_size: usize,
) -> agent_semantic_client_db::ProviderSelectorProjection {
    agent_semantic_client_db::ProviderSelectorProjection {
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
    let source = "pub fn run() {}\npub struct Record;\n";
    let captures = join_capture_projections(
        vec![
            agent_semantic_provider_transport::ProviderSyntaxQueryCapture {
                pattern_index: 0,
                capture_name: "declaration.name".to_owned(),
                native_fact_ref: "rust:item:src/lib.rs:1:1:run".to_owned(),
                source_byte_start: 7,
                source_byte_end: 10,
            },
            agent_semantic_provider_transport::ProviderSyntaxQueryCapture {
                pattern_index: 1,
                capture_name: "declaration.name".to_owned(),
                native_fact_ref: "rust:item:src/lib.rs:2:2:Record".to_owned(),
                source_byte_start: 25,
                source_byte_end: 31,
            },
        ],
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
fn query_capture_without_complete_owner_item_is_not_a_semantic_match() {
    let source = r#"pub const INSTALL: &str = "asp install plugin --codex";"#;

    let captures = join_capture_projections(
        vec![
            agent_semantic_provider_transport::ProviderSyntaxQueryCapture {
                pattern_index: 0,
                capture_name: "value".to_owned(),
                native_fact_ref: "rust:item:src/lib.rs:1:1:INSTALL".to_owned(),
                source_byte_start: 26,
                source_byte_end: source.len() as u64 - 1,
            },
        ],
        &[],
    )
    .expect("unowned parser captures are excluded");
    assert!(captures.is_empty());
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
fn tree_sitter_query_owns_one_result_model_and_one_emission_path() {
    let route = include_str!("../../../src/command/workspace_tree_sitter_query.rs");

    for forbidden in [
        "IncrementalTreeSitterState",
        "IncrementalWorkspaceQueryResult",
        "run_incremental_workspace_query",
        "render_incremental_evidence",
        "merge_tree_sitter_incremental_header",
        "executionMode=incremental",
        "\"mode\": \"incremental\"",
    ] {
        assert!(
            !route.contains(forbidden),
            "Tree-sitter query keeps a sibling incremental model or renderer `{forbidden}`"
        );
    }

    assert!(route.contains("struct TreeSitterQueryState"));
    assert!(route.contains("struct WorkspaceTreeSitterQueryResult"));
    assert_eq!(
        route.matches("render_tree_sitter_query_summary(").count(),
        2
    );
    assert_eq!(route.matches("render_tree_sitter_query_next(").count(), 2);
    assert_eq!(route.matches("[search-treesitter]").count(), 1);
    assert_eq!(route.matches("\"execution\": {").count(), 1);
}

#[test]
fn cold_tree_sitter_owner_budget_limits_provider_subprocesses_to_one_owner() {
    assert_eq!(TREE_SITTER_OWNER_BUDGET, 1);
}

#[test]
fn provider_ignored_prefixes_match_only_complete_path_components() {
    let ignored = vec![
        "./target".to_string(),
        "node_modules/".to_string(),
        ".git".to_string(),
    ];

    assert!(provider_path_is_ignored("target", &ignored));
    assert!(provider_path_is_ignored("target/debug/build.rs", &ignored));
    assert!(provider_path_is_ignored(
        "node_modules/package/index.rs",
        &ignored
    ));
    assert!(provider_path_is_ignored(".git/worktrees/demo", &ignored));
    assert!(!provider_path_is_ignored("targeted/src/lib.rs", &ignored));
    assert!(!provider_path_is_ignored("src/node_modules.rs", &ignored));
}
