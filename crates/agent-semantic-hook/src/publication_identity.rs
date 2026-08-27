//! Publication identity predicates shared by Host acceptance and black-box TestKit assertions.

use serde_json::Value;

pub fn is_typed_hook_deny(value: &Value) -> bool {
    value.get("schemaId").and_then(Value::as_str) == Some("agent.semantic-protocols.hook.decision")
        && value.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && value.get("event").and_then(Value::as_str) == Some("pre-tool")
        && value.get("decision").and_then(Value::as_str) == Some("deny")
}

pub fn is_generation_bound_hook_deny(value: &Value) -> bool {
    is_typed_hook_deny(value)
        && value
            .pointer("/fields/configRuleId")
            .and_then(Value::as_str)
            .is_some_and(|rule_id| !rule_id.is_empty())
        && value
            .pointer("/fields/hookPolicySnapshotDigest")
            .and_then(Value::as_str)
            .is_some_and(|digest| digest.starts_with("blake3-256:"))
        && value
            .pointer("/fields/hookRuntimeArtifactFingerprint")
            .and_then(Value::as_str)
            .is_some_and(|fingerprint| !fingerprint.is_empty())
        && matches!(
            value
                .pointer("/fields/hookMatcherGeneration")
                .and_then(Value::as_str),
            Some("mmap-hit" | "self-recovered")
        )
}
