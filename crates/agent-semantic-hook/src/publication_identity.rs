// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Publication identity predicates shared by Host acceptance and black-box TestKit assertions.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

pub const HOOK_RUNTIME_IDENTITY_SCHEMA_ID: &str = "agent.semantic-protocols.hook-runtime-identity";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookRuntimeIdentityReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub policy_content_digest: String,
    pub handler_elapsed_nanos: u64,
}

impl HookRuntimeIdentityReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != HOOK_RUNTIME_IDENTITY_SCHEMA_ID || self.schema_version != 1 {
            return Err("Hook Runtime identity receipt schema mismatch".to_owned());
        }
        let Some(hex) = self.policy_content_digest.strip_prefix("blake3-256:") else {
            return Err("Hook Runtime identity receipt digest is not BLAKE3".to_owned());
        };
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Hook Runtime identity receipt digest is malformed".to_owned());
        }
        Ok(())
    }
}

/// Produce the complete process-local identity without Config parsing,
/// Runtime/DB initialization, socket I/O, or workspace discovery.
pub fn hook_runtime_identity_receipt() -> Result<HookRuntimeIdentityReceipt, String> {
    let started = std::time::Instant::now();
    let policy_content_digest = crate::aot_compiler::embedded_hook_policy_content_digest()?;
    let receipt = HookRuntimeIdentityReceipt {
        schema_id: HOOK_RUNTIME_IDENTITY_SCHEMA_ID.to_owned(),
        schema_version: 1,
        policy_content_digest,
        handler_elapsed_nanos: started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn is_typed_hook_deny(value: &Value) -> bool {
    is_classic_typed_hook_deny(value) || is_aot_typed_hook_deny(value)
}

fn is_classic_typed_hook_deny(value: &Value) -> bool {
    value.get("schemaId").and_then(Value::as_str) == Some("agent.semantic-protocols.hook.decision")
        && value.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && value.get("event").and_then(Value::as_str) == Some("pre-tool")
        && value.get("decision").and_then(Value::as_str) == Some("deny")
}

fn is_aot_typed_hook_deny(value: &Value) -> bool {
    value.get("schemaId").and_then(Value::as_str) == Some("agent.semantic-protocols.hook.decision")
        && value.get("schemaVersion").and_then(Value::as_u64) == Some(1)
        && value.get("decision").and_then(Value::as_str) == Some("deny")
}

pub fn is_generation_bound_hook_deny(value: &Value) -> bool {
    is_classic_generation_bound_hook_deny(value) || is_aot_generation_bound_hook_deny(value)
}

fn is_classic_generation_bound_hook_deny(value: &Value) -> bool {
    is_classic_typed_hook_deny(value)
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
        && value
            .pointer("/fields/hookMatcherGeneration")
            .and_then(Value::as_str)
            .is_some_and(|generation| {
                generation.strip_prefix("blake3-256:").is_some_and(|hex| {
                    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            })
}

fn is_aot_generation_bound_hook_deny(value: &Value) -> bool {
    value.get("schemaId").and_then(Value::as_str) == Some("agent.semantic-protocols.hook.decision")
        && value.get("schemaVersion").and_then(Value::as_u64) == Some(1)
        && value.get("decision").and_then(Value::as_str) == Some("deny")
        && value
            .get("configRuleId")
            .and_then(Value::as_str)
            .is_some_and(|rule_id| !rule_id.is_empty())
        && value
            .get("generationDigest")
            .and_then(Value::as_str)
            .is_some_and(valid_blake3_digest)
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest
        .strip_prefix("blake3-256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
#[path = "../tests/unit/publication_identity.rs"]
mod tests;
