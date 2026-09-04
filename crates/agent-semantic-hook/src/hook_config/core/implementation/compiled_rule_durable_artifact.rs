//! Owns the versioned binary executable matcher artifact carried by Hook snapshots.

use agent_semantic_config::HookClientConfigFile;

use crate::hook_config::core::match_types::DurableCommandContainsMatcher;
use crate::hook_config::core::match_types::DurablePathGlobMatcher;

pub(super) const DURABLE_HOOK_MATCHER_SCHEMA_ID: &str =
    "agent.semantic-protocols.hook-matcher-artifact";
pub(super) const DURABLE_HOOK_MATCHER_SCHEMA_VERSION: &str = "2";

/// Normalized Hook config plus parser-compiled matcher automata.
#[derive(Clone, Debug)]
pub struct DurableHookConfigArtifact {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) config: HookClientConfigFile,
    pub(super) rule_matchers: std::collections::BTreeMap<String, DurableRuleMatcherArtifact>,
    pub(super) policy_generation_digest: String,
    pub(super) provider_projections:
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DurableRuleMatcherArtifact {
    pub(super) command_contains: DurableCommandContainsMatcher,
    pub(super) path_glob: DurablePathGlobMatcher,
    pub(super) argv_source_glob: DurablePathGlobMatcher,
    #[serde(default)]
    pub(super) profile_extension_any: Vec<String>,
    #[serde(default)]
    pub(super) profile_any: Vec<agent_semantic_config::HookClientProfileConfig>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DurableHookConfigArtifactPayload {
    schema_id: String,
    schema_version: String,
    config: HookClientConfigFile,
    rule_matchers: std::collections::BTreeMap<String, DurableRuleMatcherArtifact>,
    policy_generation_digest: String,
    provider_projections: Vec<DurableHookProviderProjection>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct DurableCommandTemplate {
    argv: Vec<String>,
    stdin_mode: Option<crate::protocol::StdinMode>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct DurableHookProviderProjection {
    language_id: agent_semantic_config::LanguageId,
    provider_id: agent_semantic_config::ProviderId,
    package_roots: Vec<String>,
    source_extensions: Vec<String>,
    config_files: Vec<String>,
    policy: crate::protocol::HookPolicy,
    playbook_route: DurableCommandTemplate,
}

impl From<&crate::protocol::CommandTemplate> for DurableCommandTemplate {
    fn from(value: &crate::protocol::CommandTemplate) -> Self {
        Self {
            argv: value.argv.clone(),
            stdin_mode: value.stdin_mode,
        }
    }
}

impl From<DurableCommandTemplate> for crate::protocol::CommandTemplate {
    fn from(value: DurableCommandTemplate) -> Self {
        Self {
            argv: value.argv,
            stdin_mode: value.stdin_mode,
        }
    }
}

impl From<&crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>
    for DurableHookProviderProjection
{
    fn from(
        value: &crate::protocol_activation::protocol_activation_manifest::HookProviderProjection,
    ) -> Self {
        Self {
            language_id: value.language_id.clone(),
            provider_id: value.provider_id.clone(),
            package_roots: value.package_roots.clone(),
            source_extensions: value.source_extensions.clone(),
            config_files: value.config_files.clone(),
            policy: value.policy.clone(),
            playbook_route: (&value.playbook_route).into(),
        }
    }
}

impl From<DurableHookProviderProjection>
    for crate::protocol_activation::protocol_activation_manifest::HookProviderProjection
{
    fn from(value: DurableHookProviderProjection) -> Self {
        Self {
            language_id: value.language_id,
            provider_id: value.provider_id,
            package_roots: value.package_roots,
            source_extensions: value.source_extensions,
            config_files: value.config_files,
            policy: value.policy,
            playbook_route: value.playbook_route.into(),
        }
    }
}

impl DurableHookConfigArtifact {
    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, String> {
        let payload = DurableHookConfigArtifactPayload {
            schema_id: self.schema_id.clone(),
            schema_version: self.schema_version.clone(),
            config: self.config.clone(),
            rule_matchers: self.rule_matchers.clone(),
            policy_generation_digest: self.policy_generation_digest.clone(),
            provider_projections: self.provider_projections.iter().map(Into::into).collect(),
        };
        let payload = postcard::to_allocvec(&payload)
            .map_err(|error| format!("encode durable Hook matcher payload: {error}"))?;
        let digest = blake3::hash(&payload);
        let mut bytes = Vec::with_capacity(digest.as_bytes().len() + payload.len());
        bytes.extend_from_slice(digest.as_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, String> {
        let (expected_digest, payload) = bytes
            .split_at_checked(32)
            .ok_or_else(|| "durable Hook matcher artifact is truncated".to_owned())?;
        if expected_digest != blake3::hash(payload).as_bytes() {
            return Err("durable Hook matcher artifact content digest mismatch".to_owned());
        }
        let payload: DurableHookConfigArtifactPayload = postcard::from_bytes(payload)
            .map_err(|error| format!("decode durable Hook matcher payload: {error}"))?;
        Ok(Self {
            schema_id: payload.schema_id,
            schema_version: payload.schema_version,
            config: payload.config,
            rule_matchers: payload.rule_matchers,
            policy_generation_digest: payload.policy_generation_digest,
            provider_projections: payload
                .provider_projections
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}
