use std::path::Path;

use super::{
    ProviderExecution, ResolvedProvider, normalize_project_path, project_child_path,
    provider_ignores_path, provider_supports_source_file, relative_project_path, scoped_child_path,
};

#[test]
fn scoped_child_path_rejects_absolute_and_parent_escape() {
    let root = Path::new("/repo");

    assert_eq!(project_child_path(root, "."), Some(root.to_path_buf()));
    assert_eq!(project_child_path(root, ""), Some(root.to_path_buf()));
    assert_eq!(
        scoped_child_path(root, "src/lib.rs"),
        Some(root.join("src/lib.rs"))
    );
    assert_eq!(scoped_child_path(root, "../outside.rs"), None);
    assert_eq!(scoped_child_path(root, "/tmp/outside.rs"), None);
}

#[test]
fn relative_and_normalized_project_paths_use_slash_form() {
    let root = Path::new("/repo");

    assert_eq!(
        relative_project_path(root, Path::new("/repo/src/lib.rs")),
        "src/lib.rs"
    );
    assert_eq!(normalize_project_path(".\\target\\debug"), "target/debug");
}

#[test]
fn provider_source_extension_matching_is_case_insensitive() {
    let provider = provider().with_source_extensions(vec![".rs".to_string(), "toml".to_string()]);

    assert!(provider_supports_source_file(
        &provider,
        Path::new("src/LIB.RS")
    ));
    assert!(provider_supports_source_file(
        &provider,
        Path::new("Cargo.toml")
    ));
    assert!(!provider_supports_source_file(
        &provider,
        Path::new("README.md")
    ));
}

#[test]
fn provider_ignore_prefix_matching_is_project_relative() {
    let provider = provider()
        .with_ignored_path_prefixes(vec!["target".to_string(), ".cache/generated".to_string()]);
    let root = Path::new("/repo");

    assert!(provider_ignores_path(
        root,
        &provider,
        Path::new("/repo/target/debug/lib.rlib")
    ));
    assert!(provider_ignores_path(
        root,
        &provider,
        Path::new("/repo/.cache/generated/file.rs")
    ));
    assert!(!provider_ignores_path(
        root,
        &provider,
        Path::new("/repo/src/targeted.rs")
    ));
}

fn provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: "rust".into(),
        provider_id: "rs-harness".into(),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: Vec::new(),
        source_roots: Vec::new(),
        config_files: Vec::new(),
        source_extensions: Vec::new(),
        ignored_path_prefixes: Vec::new(),
    }
}

trait ProviderFixtureExt {
    fn with_source_extensions(self, source_extensions: Vec<String>) -> Self;
    fn with_ignored_path_prefixes(self, ignored_path_prefixes: Vec<String>) -> Self;
}

impl ProviderFixtureExt for ResolvedProvider {
    fn with_source_extensions(mut self, source_extensions: Vec<String>) -> Self {
        self.source_extensions = source_extensions;
        self
    }

    fn with_ignored_path_prefixes(mut self, ignored_path_prefixes: Vec<String>) -> Self {
        self.ignored_path_prefixes = ignored_path_prefixes;
        self
    }
}
