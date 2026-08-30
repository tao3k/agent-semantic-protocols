//! Assertions that bind a host-observed Hook decision to the exact publication under test.

use std::fmt;

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstalledHookExpectation<'a> {
    pub event: &'a str,
    pub decision: &'a str,
    pub config_rule_id: Option<&'a str>,
    pub matcher_generation: &'a str,
    pub policy_snapshot_digest: &'a str,
    pub runtime_artifact_fingerprint: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledHookReceiptMismatch {
    field: &'static str,
    expected: String,
    observed: Option<String>,
}

impl InstalledHookReceiptMismatch {
    fn new(field: &'static str, expected: impl Into<String>, observed: Option<&Value>) -> Self {
        Self {
            field,
            expected: expected.into(),
            observed: observed.and_then(Value::as_str).map(str::to_owned),
        }
    }
}

impl fmt::Display for InstalledHookReceiptMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "installed Hook receipt field `{}` mismatch: expected `{}`, observed {}",
            self.field,
            self.expected,
            self.observed
                .as_deref()
                .unwrap_or("<missing-or-non-string>")
        )
    }
}

impl std::error::Error for InstalledHookReceiptMismatch {}

pub fn verify_installed_hook_receipt(
    receipt: &Value,
    expected: InstalledHookExpectation<'_>,
) -> Result<(), InstalledHookReceiptMismatch> {
    if expected.decision == "deny" && !agent_semantic_hook::is_generation_bound_hook_deny(receipt) {
        return Err(InstalledHookReceiptMismatch::new(
            "/",
            "generation-bound typed Hook deny",
            None,
        ));
    }
    require_string(receipt, "/event", expected.event)?;
    require_string(receipt, "/decision", expected.decision)?;
    require_string(
        receipt,
        "/fields/hookMatcherGeneration",
        expected.matcher_generation,
    )?;
    require_string(
        receipt,
        "/fields/hookPolicySnapshotDigest",
        expected.policy_snapshot_digest,
    )?;
    require_string(
        receipt,
        "/fields/hookRuntimeArtifactFingerprint",
        expected.runtime_artifact_fingerprint,
    )?;

    match expected.config_rule_id {
        Some(rule_id) => require_string(receipt, "/fields/configRuleId", rule_id),
        None if receipt.pointer("/fields/configRuleId").is_none() => Ok(()),
        None => Err(InstalledHookReceiptMismatch::new(
            "/fields/configRuleId",
            "<absent>",
            receipt.pointer("/fields/configRuleId"),
        )),
    }
}

fn require_string(
    receipt: &Value,
    pointer: &'static str,
    expected: &str,
) -> Result<(), InstalledHookReceiptMismatch> {
    let observed = receipt.pointer(pointer);
    if observed.and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(InstalledHookReceiptMismatch::new(
            pointer, expected, observed,
        ))
    }
}

#[cfg(test)]
#[path = "../tests/unit/installed_publication.rs"]
mod tests;
