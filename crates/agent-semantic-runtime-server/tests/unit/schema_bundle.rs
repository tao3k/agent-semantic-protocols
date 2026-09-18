// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuntimeSchemaBundleCatalog;

#[test]
fn embedded_bundle_binding_is_v1_deterministic_and_complete() {
    let catalog = RuntimeSchemaBundleCatalog::load_embedded().expect("embedded schema catalog");
    let once = catalog
        .binding_digest(["rust"])
        .expect("Rust schema bundle binding");
    let duplicated = catalog
        .binding_digest(["rust", "rust"])
        .expect("deduplicated Rust schema bundle binding");
    assert_eq!(once, duplicated);
    assert!(once.starts_with("blake3-256:"));

    let missing = catalog
        .binding_digest(["not-a-registered-language"])
        .expect_err("missing required schema bundle must fail closed");
    assert!(
        missing.contains("absent for required language"),
        "{missing}"
    );
}

#[test]
fn execution_closure_entries_come_from_the_embedded_runtime_catalog() {
    let catalog = RuntimeSchemaBundleCatalog::load_embedded().expect("embedded schema catalog");
    let entries = catalog
        .execution_closure_entries()
        .expect("Runtime execution closure entries");
    let language_ids = entries
        .iter()
        .map(|entry| entry.language_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        language_ids,
        [
            "gerbil-scheme",
            "julia",
            "md",
            "org",
            "python",
            "rust",
            "typescript",
        ]
    );
}
