// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::cargo_build_gate_cache_root;

#[test]
fn cargo_cache_authority_is_stable_across_package_build_hashes() {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let first = cargo_build_gate_cache_root(
        project_root,
        std::path::Path::new("/tmp/target/debug/build/member-aaaa/out"),
    )
    .unwrap();
    let second = cargo_build_gate_cache_root(
        project_root,
        std::path::Path::new("/tmp/target/debug/build/member-bbbb/out"),
    )
    .unwrap();

    assert_eq!(first, second);
    assert!(first.starts_with("/tmp/target/debug/build"));
    assert!(first.parent().is_some_and(|parent| parent.ends_with("v1")));
    assert_eq!(
        first
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::len),
        Some(32)
    );
}

#[test]
fn cargo_cache_authority_rejects_non_cargo_output_layout() {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(cargo_build_gate_cache_root(project_root, std::path::Path::new("/tmp/out")).is_err());
}
