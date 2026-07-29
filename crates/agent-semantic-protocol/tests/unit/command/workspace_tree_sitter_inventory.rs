use std::path::{Path, PathBuf};

use super::{ProviderProjectScope, selector_matches_path};

#[test]
fn provider_scope_intersects_project_markers_package_roots_and_source_roots() {
    let candidates = [
        "Cargo.toml",
        "src/lib.rs",
        "docs/example.rs",
        "crates/demo/Cargo.toml",
        "crates/demo/src/lib.rs",
        "crates/demo/tests/query.rs",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
    let scope = ProviderProjectScope::discover(
        &candidates,
        &["Cargo.toml".to_string()],
        &["crates/*".to_string()],
        &["src".to_string(), "tests".to_string()],
    );

    assert!(scope.contains_source(Path::new("src/lib.rs")));
    assert!(scope.contains_source(Path::new("crates/demo/src/lib.rs")));
    assert!(scope.contains_source(Path::new("crates/demo/tests/query.rs")));
    assert!(!scope.contains_source(Path::new("docs/example.rs")));
}

#[test]
fn provider_scope_falls_back_to_declared_source_roots_without_project_marker() {
    let candidates = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("generated/cache.rs"),
    ];
    let scope = ProviderProjectScope::discover(&candidates, &[], &[], &["src".to_string()]);

    assert!(scope.contains_source(Path::new("src/lib.rs")));
    assert!(!scope.contains_source(Path::new("generated/cache.rs")));
}

#[test]
fn dot_package_root_explicitly_owns_the_provider_workspace_root() {
    let candidates = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("docs/example.rs"),
    ];
    let scope =
        ProviderProjectScope::discover(&candidates, &[], &[".".to_string()], &["src".to_string()]);

    assert!(scope.contains_source(Path::new("src/lib.rs")));
    assert!(!scope.contains_source(Path::new("docs/example.rs")));
}

#[test]
fn selector_matching_is_path_aware_and_supports_provider_patterns() {
    assert!(selector_matches_path(
        "Cargo.toml",
        "crates/demo/Cargo.toml"
    ));
    assert!(selector_matches_path("crates/*", "crates/demo"));
    assert!(!selector_matches_path("src", "src-generated"));
}
