// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::LanguageFileSpec;
use crate::language_file_spec;
use crate::language_neutral_search_file_spec;

#[test]
fn programming_language_scope_is_not_reconstructed_from_filenames() {
    let rust = language_file_spec("rust");

    assert!(rust.extensions().is_empty());
    assert!(!rust.matches(std::path::Path::new("src/lib.rs")));
    assert!(rust.config_filenames().is_empty());
    assert!(!rust.is_config_path(std::path::Path::new("Cargo.toml")));
}

#[test]
fn language_neutral_spec_is_fail_closed_without_runtime_scope() {
    let spec = language_neutral_search_file_spec();

    assert!(!spec.matches(std::path::Path::new("notes.org")));
    assert!(!spec.matches(std::path::Path::new("README.md")));
    assert!(!spec.matches(std::path::Path::new("src/lib.rs")));
    assert!(!spec.matches(std::path::Path::new("package.json")));
    assert!(spec.project_markers().is_empty());
}

#[test]
fn runtime_scope_is_the_only_source_of_language_file_facts() {
    let spec = LanguageFileSpec::from_runtime_scope(
        vec!["rs".to_owned()],
        vec!["Cargo.toml".to_owned()],
        vec!["Cargo.toml".to_owned()],
        vec!["Cargo.lock".to_owned()],
    );

    assert!(spec.matches(std::path::Path::new("src/lib.rs")));
    assert!(spec.is_config_path(std::path::Path::new("Cargo.toml")));
    assert_eq!(spec.project_markers(), ["Cargo.toml"]);
    assert_eq!(spec.dependency_markers(), ["Cargo.lock"]);
}
