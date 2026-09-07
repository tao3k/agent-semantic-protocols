// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::file_hash_belongs_to_complete_generation;
use std::collections::BTreeSet;

#[test]
fn complete_successor_rejects_orphan_file_hashes_but_keeps_scope_evidence() {
    let owners = BTreeSet::from(["src/owned.rs".to_owned()]);

    assert!(file_hash_belongs_to_complete_generation(
        "src/owned.rs",
        &owners
    ));
    assert!(file_hash_belongs_to_complete_generation(
        "@scope/selector-generation/src/owned.rs",
        &owners
    ));
    assert!(!file_hash_belongs_to_complete_generation(
        "src/removed.rs",
        &owners
    ));
}
