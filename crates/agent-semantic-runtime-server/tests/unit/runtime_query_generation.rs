// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{execution_source_root_matches_resident, topology_owner_root_fallback_scope};

#[test]
fn canonical_execution_source_root_matches_resident_content_component() {
    let content = "a".repeat(64);
    assert!(execution_source_root_matches_resident(
        &format!("blake3-256:{content}"),
        &content,
    ));
    assert!(execution_source_root_matches_resident(&content, &content));
    assert!(!execution_source_root_matches_resident(
        &format!("blake3-256:{}", "b".repeat(64)),
        &content,
    ));
}

#[test]
fn byte_first_topology_fills_every_owner_without_a_parser_item_with_an_owner_root() {
    let owner_scope = ["src/detailed.rs", "src/edited.rs", "src/plain.rs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let detailed_owners = ["src/detailed.rs"].into_iter().collect();
    let explicit_owner_roots = ["src/plain.rs"].into_iter().map(str::to_owned).collect();

    assert_eq!(
        topology_owner_root_fallback_scope(&owner_scope, &detailed_owners, explicit_owner_roots,),
        ["src/edited.rs", "src/plain.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}
