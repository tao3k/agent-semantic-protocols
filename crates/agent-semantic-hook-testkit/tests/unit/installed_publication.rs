// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde_json::json;

use super::InstalledHookExpectation;
use super::verify_installed_hook_receipt;

const GENERATION: &str =
    "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const EXPECTED: InstalledHookExpectation<'static> = InstalledHookExpectation {
    event: "pre-tool",
    decision: "deny",
    config_rule_id: Some("route-read-to-asp-languages"),
    matcher_generation: GENERATION,
    policy_snapshot_digest: "blake3-256:policy",
    runtime_artifact_fingerprint: "blake3-256:artifact",
};

fn exact_receipt() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": "1",
        "event": "pre-tool",
        "decision": "deny",
        "fields": {
            "configRuleId": "route-read-to-asp-languages",
            "hookMatcherGeneration": GENERATION,
            "hookPolicySnapshotDigest": "blake3-256:policy",
            "hookRuntimeArtifactFingerprint": "blake3-256:artifact"
        }
    })
}

#[test]
fn exact_installed_generation_is_accepted() {
    verify_installed_hook_receipt(&exact_receipt(), EXPECTED).expect("exact publication");
}

#[test]
fn observed_deny_without_publication_identity_is_rejected() {
    let receipt = json!({"event": "pre-tool", "decision": "deny", "fields": {}});
    assert!(verify_installed_hook_receipt(&receipt, EXPECTED).is_err());
}

#[test]
fn stale_policy_snapshot_is_rejected() {
    let mut receipt = exact_receipt();
    receipt["fields"]["hookPolicySnapshotDigest"] = json!("blake3-256:stale");
    assert!(verify_installed_hook_receipt(&receipt, EXPECTED).is_err());
}

#[test]
fn wrong_rule_is_rejected() {
    let mut receipt = exact_receipt();
    receipt["fields"]["configRuleId"] = json!("another-rule");
    assert!(verify_installed_hook_receipt(&receipt, EXPECTED).is_err());
}

#[test]
fn post_tool_observation_cannot_satisfy_pre_tool_enforcement() {
    let mut receipt = exact_receipt();
    receipt["event"] = json!("post-tool");
    assert!(verify_installed_hook_receipt(&receipt, EXPECTED).is_err());
}
