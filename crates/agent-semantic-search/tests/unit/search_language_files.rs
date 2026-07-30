use crate::{language_file_spec, language_neutral_search_file_spec};

#[test]
fn programming_language_scope_is_not_reconstructed_from_filenames() {
    let rust = language_file_spec("rust");

    assert!(rust.extensions().is_empty());
    assert!(!rust.matches(std::path::Path::new("src/lib.rs")));
    assert!(rust.config_filenames().is_empty());
    assert!(!rust.is_config_path(std::path::Path::new("Cargo.toml")));
}

#[test]
fn language_neutral_spec_uses_registered_document_resolution_only() {
    let spec = language_neutral_search_file_spec();

    assert!(spec.matches(std::path::Path::new("notes.org")));
    assert!(spec.matches(std::path::Path::new("README.md")));
    assert!(!spec.matches(std::path::Path::new("src/lib.rs")));
    assert!(!spec.matches(std::path::Path::new("package.json")));
    assert!(spec.project_markers().is_empty());
}
