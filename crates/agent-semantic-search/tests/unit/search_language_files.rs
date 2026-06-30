use crate::{language_file_spec, language_neutral_search_file_spec};

#[test]
fn language_file_spec_comes_from_provider_manifest_defaults() {
    let rust = language_file_spec("rust");

    assert!(
        rust.extensions()
            .iter()
            .any(|extension| extension.trim_start_matches('.') == "rs")
    );
    assert!(
        rust.matches(std::path::Path::new("src/lib.rs")),
        "rust provider default extensions should match Rust owners"
    );
    assert!(
        rust.config_filenames()
            .iter()
            .any(|filename| filename == "Cargo.toml"),
        "rust provider config defaults should include Cargo.toml"
    );
    assert!(rust.is_config_path(std::path::Path::new("Cargo.toml")));
}

#[test]
fn language_neutral_spec_merges_provider_manifest_defaults() {
    let spec = language_neutral_search_file_spec();

    assert!(spec.matches(std::path::Path::new("src/lib.rs")));
    assert!(spec.matches(std::path::Path::new("package.json")));
    assert!(
        spec.project_markers()
            .iter()
            .any(|marker| marker == "Cargo.toml")
    );
}
