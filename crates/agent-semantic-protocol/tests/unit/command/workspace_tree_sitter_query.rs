use std::collections::BTreeMap;

use super::{WorkspaceTreeSitterRequest, collect_workspace_captures, registered_source_path};

#[test]
fn workspace_query_request_requires_query_source_without_selector() {
    let request = WorkspaceTreeSitterRequest::parse(&[
        "query".to_string(),
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
}

#[test]
fn runtime_executes_predicates_without_capture_text_projection() {
    let language =
        agent_semantic_tree_sitter::registered_language_grammar("rust").expect("Rust grammar");
    let query = agent_semantic_tree_sitter::compile_native_query_source(
        &language,
        r#"((string_literal) @value (#match? @value "asp install plugin --codex"))"#,
    )
    .expect("compile query");
    let source_blobs = BTreeMap::from([
        (
            "src/lib.rs".to_string(),
            br#"pub const INSTALL: &str = "asp install plugin --codex";"#.to_vec(),
        ),
        (
            "src/ignored.py".to_string(),
            br#"VALUE = "asp install plugin --codex""#.to_vec(),
        ),
    ]);
    let (captures, total) =
        collect_workspace_captures(&language, &query, &source_blobs, &["rs".to_string()])
            .expect("execute query");
    assert_eq!(total, 1);
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].owner_path, "src/lib.rs");
    assert_eq!(captures[0].node_kind, "string_literal");
    assert_eq!(captures[0].capture_name, "value");
}

#[test]
fn runtime_executes_one_capture_across_multiple_node_kinds() {
    let language =
        agent_semantic_tree_sitter::registered_language_grammar("rust").expect("Rust grammar");
    let query = agent_semantic_tree_sitter::compile_native_query_source(
        &language,
        "[(function_item name: (identifier) @declaration.name) (struct_item name: (type_identifier) @declaration.name) (enum_item name: (type_identifier) @declaration.name) (trait_item name: (type_identifier) @declaration.name) (type_item name: (type_identifier) @declaration.name)]",
    )
    .expect("compile query");
    let source_blobs = BTreeMap::from([(
        "src/lib.rs".to_string(),
        b"pub fn run() {}\npub struct Record;\npub enum Choice { A }\npub trait Worker {}\npub type Alias = usize;\n".to_vec(),
    )]);
    let (captures, total) =
        collect_workspace_captures(&language, &query, &source_blobs, &[".rs".to_string()])
            .expect("execute query");
    assert_eq!(total, 5);
    assert_eq!(captures.len(), 5);
    assert!(
        captures
            .iter()
            .all(|capture| capture.capture_name == "declaration.name")
    );
    assert!(registered_source_path("src/lib.rs", &["rs".to_string()]));
    assert!(!registered_source_path("src/lib.py", &["rs".to_string()]));
}
