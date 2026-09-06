// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::source_query_keys;

#[test]
fn query_keys_include_snake_and_kebab_components() {
    let keys = source_query_keys(
        "src/report-chain.rs",
        "fn topology_membership_report_chain_request_policy() {}",
    );

    for expected in [
        "topology",
        "membership",
        "report",
        "chain",
        "request",
        "policy",
    ] {
        assert!(keys.iter().any(|key| key == expected), "missing {expected}");
    }
}

#[test]
fn gerbil_form_heads_are_resident_source_index_terms() {
    let keys = source_query_keys("src/demo.ss", "(import :std/sugar)\n(def foo (x) x)");
    for expected in ["import", "std", "sugar", "def", "foo"] {
        assert!(keys.iter().any(|key| key == expected), "missing {expected}");
    }
}
