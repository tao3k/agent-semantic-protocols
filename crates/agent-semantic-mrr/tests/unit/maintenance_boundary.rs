// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;

#[test]
fn mrr_is_the_only_workspace_crate_with_direct_kernel_or_gql_dependencies() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_root = crate_root.parent().expect("workspace crates root");
    let forbidden = [
        "ascent.workspace = true",
        "gql-catalog.workspace = true",
        "gql-compiler.workspace = true",
        "gql-ir.workspace = true",
        "meta-relational-reasoning.workspace = true",
    ];
    for entry in fs::read_dir(crates_root).expect("list workspace crates") {
        let path = entry.expect("crate entry").path();
        if path == crate_root || !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = fs::read_to_string(&manifest_path).expect("read crate manifest");
        for dependency in forbidden {
            assert!(
                !manifest.contains(dependency),
                "{} bypasses agent-semantic-mrr with {dependency}",
                manifest_path.display()
            );
        }
    }
}
