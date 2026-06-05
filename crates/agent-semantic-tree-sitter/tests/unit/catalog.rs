use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    SyntaxCatalogDescriptor, builtin_catalog_source, compile_query_abi_source,
    extract_capture_names, load_grammar_profile, load_syntax_catalog,
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
fn extract_capture_names_ignores_comments_and_predicate_strings() {
    let source = r#"
        ; @comment.capture must not be part of the ABI
        ((identifier) @local.name
          (#match? @local.name "@string.capture"))
    "#;

    assert_eq!(extract_capture_names(source), vec!["local.name"]);
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
fn load_catalog_rejects_malformed_query_source() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("agent-semantic-tree-sitter-test-{nonce}"));
    let catalog_dir = dir.join("tree-sitter/tree-sitter-rust");
    fs::create_dir_all(&catalog_dir).expect("mkdir");
    fs::write(
        catalog_dir.join("broken.scm"),
        "(function_item name: (identifier) @function.name",
    )
    .expect("write catalog");
    let descriptor = SyntaxCatalogDescriptor {
        id: "broken".to_string(),
        path: PathBuf::from("tree-sitter/tree-sitter-rust/broken.scm"),
        declared_captures: vec!["function.name".to_string()],
    };

    let error = load_syntax_catalog(&dir, &descriptor).expect_err("malformed query source");

    assert!(
        error.contains("failed to compile syntax query catalog"),
        "{error}"
    );
    assert!(error.contains("unclosed query pattern"), "{error}");

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

#[test]
fn built_in_language_catalogs_compile_to_abi_plans() {
    let catalogs = [
        ("rust", "calls", "call.target"),
        ("rust", "cfg", "attribute.name"),
        ("rust", "declarations", "function.name"),
        ("rust", "imports", "import.path"),
        ("rust", "macros", "macro.name"),
        ("typescript", "calls", "call.target"),
        ("typescript", "declarations", "function.name"),
        ("typescript", "imports", "import.source"),
        ("python", "calls", "call.target"),
        ("python", "control-flow", "control.loop"),
        ("python", "declarations", "function.name"),
        ("python", "decorators", "decorator.target"),
        ("python", "imports", "import.path"),
    ];

    for (language_id, catalog_id, expected_capture) in catalogs {
        let source = builtin_catalog_source(language_id.into(), catalog_id.into())
            .expect("built-in catalog source");
        let plan = compile_query_abi_source(source).expect("built-in catalog ABI plan");

        assert!(
            plan.captures
                .iter()
                .any(|capture| capture == expected_capture),
            "{language_id}:{catalog_id} missing {expected_capture}: {:?}",
            plan.captures
        );
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}
