use super::is_generation_bound_hook_deny;
use serde_json::json;

#[test]
fn aot_reader_deny_is_generation_bound_by_compiled_policy_digest() {
    let decision = json!({
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": 1,
        "decision": "deny",
        "configRuleId": "route-read-to-asp-languages",
        "generationDigest": format!("blake3-256:{}", "a".repeat(64)),
    });
    assert!(is_generation_bound_hook_deny(&decision));
}

#[test]
fn aot_reader_deny_without_rule_or_generation_is_not_bound() {
    assert!(!is_generation_bound_hook_deny(&json!({
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": 1,
        "decision": "deny",
    })));
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
