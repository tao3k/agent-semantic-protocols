// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::execution_source_root_matches_resident;

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
