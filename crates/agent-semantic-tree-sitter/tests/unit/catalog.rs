use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    SyntaxCatalogDescriptor, extract_capture_names, load_grammar_profile, load_syntax_catalog,
};

#[test]
fn extracts_and_normalizes_capture_names_from_scm() {
    let source = "(call_expression function: (_) @call.target) @call.expression\n\
                  (call_expression field: (field_identifier) @call.method)";

    assert_eq!(
        extract_capture_names(source),
        vec!["call.expression", "call.method", "call.target"]
    );
}

#[test]
fn loaded_catalog_keeps_declared_and_discovered_captures_separate() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("agent-semantic-tree-sitter-test-{nonce}"));
    let catalog_dir = dir.join("tree-sitter/tree-sitter-rust");
    fs::create_dir_all(&catalog_dir).expect("mkdir");
    fs::write(
        catalog_dir.join("calls.scm"),
        "(call_expression function: (_) @call.target) @call.expression",
    )
    .expect("write catalog");
    let descriptor = SyntaxCatalogDescriptor {
        id: "calls".to_string(),
        path: PathBuf::from("tree-sitter/tree-sitter-rust/calls.scm"),
        declared_captures: vec!["call.target".to_string(), "call.expression".to_string()],
    };

    let loaded = load_syntax_catalog(&dir, &descriptor).expect("catalog");

    assert_eq!(loaded.id, "calls");
    assert_eq!(
        loaded.declared_captures,
        vec!["call.expression", "call.target"]
    );
    assert_eq!(
        loaded.discovered_captures,
        vec!["call.expression", "call.target"]
    );
    assert!(loaded.fingerprint.starts_with("syntax-catalog:"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn loads_real_rust_provider_calls_catalog() {
    let workspace_root = workspace_root();
    let descriptor = SyntaxCatalogDescriptor {
        id: "calls".to_string(),
        path: PathBuf::from(
            "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust/queries/calls.scm",
        ),
        declared_captures: vec![
            "call.expression".to_string(),
            "call.target".to_string(),
            "call.method".to_string(),
        ],
    };

    let loaded = load_syntax_catalog(&workspace_root, &descriptor).expect("catalog");

    assert_eq!(loaded.id, "calls");
    assert_eq!(
        loaded.declared_captures,
        vec!["call.expression", "call.method", "call.target"]
    );
    assert_eq!(
        loaded.discovered_captures,
        vec!["call.expression", "call.method", "call.target"]
    );
    assert!(loaded.source.contains("@call.target"));
}

#[test]
fn loads_real_rust_provider_grammar_profile() {
    let workspace_root = workspace_root();
    let profile_path = PathBuf::from(
        "languages/rust-lang-project-harness/tree-sitter/tree-sitter-rust/grammar-profile.json",
    );

    let profile = load_grammar_profile(&workspace_root, profile_path.clone()).expect("profile");

    assert_eq!(profile.path, profile_path);
    assert!(profile.source.contains("tree-sitter-rust"));
    assert!(profile.source.contains("corpus-profile.json"));
    assert!(profile.fingerprint.starts_with("grammar-profile:"));
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}
