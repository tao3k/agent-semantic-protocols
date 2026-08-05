//! Owns the versioned binary executable matcher artifact carried by Hook snapshots.

use agent_semantic_config::HookClientConfigFile;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::hook_config::core::match_types::{
    DurableCommandContainsMatcher, DurablePathGlobMatcher,
};

pub(super) const DURABLE_HOOK_MATCHER_SCHEMA_ID: &str =
    "agent.semantic-protocols.hook-matcher-artifact";
pub(super) const DURABLE_HOOK_MATCHER_SCHEMA_VERSION: &str = "1";

/// Normalized Hook config plus parser-compiled matcher automata.
#[derive(Clone, Debug)]
pub struct DurableHookConfigArtifact {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) config: HookClientConfigFile,
    pub(super) rule_matchers: std::collections::BTreeMap<String, DurableRuleMatcherArtifact>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DurableRuleMatcherArtifact {
    pub(super) command_contains: DurableCommandContainsMatcher,
    pub(super) path_glob: DurablePathGlobMatcher,
    pub(super) argv_source_glob: DurablePathGlobMatcher,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DurableHookConfigArtifactPayload {
    schema_id: String,
    schema_version: String,
    config: HookClientConfigFile,
    rule_matchers: std::collections::BTreeMap<String, DurableRuleMatcherArtifact>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DurableHookConfigArtifactEnvelope {
    content_digest: String,
    payload: Vec<u8>,
}

impl serde::Serialize for DurableHookConfigArtifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let payload = DurableHookConfigArtifactPayload {
            schema_id: self.schema_id.clone(),
            schema_version: self.schema_version.clone(),
            config: self.config.clone(),
            rule_matchers: self.rule_matchers.clone(),
        };
        let payload = postcard::to_allocvec(&payload).map_err(serde::ser::Error::custom)?;
        let envelope = DurableHookConfigArtifactEnvelope {
            content_digest: format!("sha256:{:x}", Sha256::digest(&payload)),
            payload,
        };
        let bytes = postcard::to_allocvec(&envelope).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }
}

impl<'de> serde::Deserialize<'de> for DurableHookConfigArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let encoded = <String as serde::Deserialize>::deserialize(deserializer)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)?;
        let envelope: DurableHookConfigArtifactEnvelope =
            postcard::from_bytes(&bytes).map_err(serde::de::Error::custom)?;
        let actual_digest = format!("sha256:{:x}", Sha256::digest(&envelope.payload));
        if envelope.content_digest != actual_digest {
            return Err(serde::de::Error::custom(
                "durable Hook matcher artifact content digest mismatch",
            ));
        }
        let payload: DurableHookConfigArtifactPayload =
            postcard::from_bytes(&envelope.payload).map_err(serde::de::Error::custom)?;
        Ok(Self {
            schema_id: payload.schema_id,
            schema_version: payload.schema_version,
            config: payload.config,
            rule_matchers: payload.rule_matchers,
        })
    }
}
