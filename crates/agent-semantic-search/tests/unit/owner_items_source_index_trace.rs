use std::path::PathBuf;

use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSourceKind,
};

use crate::owner_items_source_index_trace::{
    OwnerItemsSourceIndexTrace, OwnerItemsSourceIndexTraceStream,
    render_owner_items_source_index_lookup_trace, source_index_owner_query,
};

#[test]
fn owner_items_source_index_hit_renders_to_stdout_and_handles() {
    let render = OwnerItemsSourceIndexTrace::new(
        "|sourceIndex status=hit".to_string(),
        Some(ClientDbSourceIndexLookupState::Hit),
    )
    .render();

    assert_eq!(render.stream, OwnerItemsSourceIndexTraceStream::Stdout);
    assert_eq!(render.line, "|sourceIndex status=hit");
}

#[test]
fn owner_items_source_index_miss_renders_to_stderr_without_handling() {
    let render = OwnerItemsSourceIndexTrace::new(
        "|sourceIndex status=miss".to_string(),
        Some(ClientDbSourceIndexLookupState::Miss),
    )
    .render();

    assert_eq!(render.stream, OwnerItemsSourceIndexTraceStream::Stderr);
    assert_eq!(render.line, "|sourceIndex status=miss");
}

#[test]
fn owner_items_source_index_busy_renders_to_stdout_and_handles() {
    let render = OwnerItemsSourceIndexTrace::new(
        "|sourceIndex status=busy".to_string(),
        Some(ClientDbSourceIndexLookupState::Busy),
    )
    .render();

    assert_eq!(render.stream, OwnerItemsSourceIndexTraceStream::Stdout);
    assert_eq!(render.line, "|sourceIndex status=busy");
}

#[test]
fn owner_items_source_index_missing_db_renders_to_stdout_and_handles() {
    let render = OwnerItemsSourceIndexTrace::new(
        "|sourceIndex status=missing-db".to_string(),
        Some(ClientDbSourceIndexLookupState::MissingDb),
    )
    .render();

    assert_eq!(render.stream, OwnerItemsSourceIndexTraceStream::Stdout);
    assert_eq!(render.line, "|sourceIndex status=missing-db");
}

#[test]
fn owner_items_source_index_empty_index_renders_to_stdout_and_handles() {
    let render = OwnerItemsSourceIndexTrace::new(
        "|sourceIndex status=empty-index".to_string(),
        Some(ClientDbSourceIndexLookupState::EmptyIndex),
    )
    .render();

    assert_eq!(render.stream, OwnerItemsSourceIndexTraceStream::Stdout);
    assert_eq!(render.line, "|sourceIndex status=empty-index");
}

#[test]
fn owner_items_source_index_lookup_trace_names_missing_db() {
    let lookup = ClientDbSourceIndexLookupResult {
        db_path: PathBuf::from("client.turso"),
        state: ClientDbSourceIndexLookupState::MissingDb,
        candidates: Vec::new(),
    };
    let line = render_owner_items_source_index_lookup_trace("src/lib.rs", &lookup);

    assert!(line.contains("status=missing-db"));
    assert!(line.contains("reason=sourceIndex:missing-db"));
    assert!(line.contains("source=source-index"));
}

#[test]
fn owner_items_source_index_lookup_trace_projects_hit_path() {
    let lookup = ClientDbSourceIndexLookupResult {
        db_path: PathBuf::from("client.turso"),
        state: ClientDbSourceIndexLookupState::Hit,
        candidates: vec![ClientDbSourceIndexCandidate {
            path: "src/lib.rs".to_string(),
            language_id: None,
            provider_id: None,
            source_kind: ClientDbSourceIndexSourceKind::File,
            line_count: Some(12),
            query_keys: vec!["lib".to_string()],
            selector_proof: None,
        }],
    };
    let line = render_owner_items_source_index_lookup_trace("src/lib.rs", &lookup);

    assert!(line.contains("status=hit"));
    assert!(line.contains("path=src/lib.rs"));
}

#[test]
fn source_index_owner_query_uses_project_relative_path() {
    let query =
        source_index_owner_query(&PathBuf::from("/repo"), &PathBuf::from("/repo/src/lib.rs"));

    assert_eq!(query, "src/lib.rs");
}
