use std::path::Path;

use super::RuntimeProvider;
use super::normalize_project_path;
use super::project_child_path;
use super::provider_supports_source_file;
use super::relative_project_path;
use super::scoped_child_path;
use super::test_support::runtime_provider;

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

fn provider() -> RuntimeProvider {
    runtime_provider()
}

trait ProviderFixtureExt {
    fn with_source_extensions(self, source_extensions: Vec<String>) -> Self;
}

impl ProviderFixtureExt for RuntimeProvider {
    fn with_source_extensions(mut self, source_extensions: Vec<String>) -> Self {
        self.source_extensions = source_extensions;
        self
    }
}
