//! Activation manifest loading and provider/source resolution.

use crate::protocol::{
    AgentHookError, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION,
};

use super::digest::provider_manifest_digest;
use super::protocol_activation_manifest::{
    ActivatedProvider, HookActivation, HookRuntime, ProviderManifest,
};
use super::provider_query_pack::{
    validate_query_pack_descriptor, validate_semantic_facts_descriptor,
};

fn expect_field(field: &str, actual: &str, expected: &str) -> Result<(), AgentHookError> {
    if actual == expected {
        Ok(())
    } else {
        Err(AgentHookError::InvalidActivationConfig(format!(
            "{field} must be `{expected}`, got `{actual}`"
        )))
    }
}

/// Parse, validate, and resolve activation JSON against provider manifests.
pub fn parse_activation(
    input: &str,
    manifests: &[ProviderManifest],
) -> Result<HookRuntime, AgentHookError> {
    let activation: HookActivation =
        serde_json::from_str(input).map_err(AgentHookError::InvalidActivation)?;
    activation.validate_protocol()?;
    resolve_activation(&activation, manifests)
}

fn resolve_activation(
    activation: &HookActivation,
    manifests: &[ProviderManifest],
) -> Result<HookRuntime, AgentHookError> {
    let mut providers = Vec::new();
    for activated in &activation.providers {
        let manifest = manifests
            .iter()
            .find(|manifest| manifest.manifest_id == activated.manifest_id)
            .ok_or_else(|| {
                AgentHookError::InvalidActivationConfig(format!(
                    "unknown provider manifest: {}",
                    activated.manifest_id
                ))
            })?;
        manifest.validate_protocol()?;
        validate_semantic_facts_descriptor(manifest)?;
        validate_query_pack_descriptor(manifest)?;
        let expected_digest = provider_manifest_digest(manifest)?;
        if activated.manifest_digest != expected_digest {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "provider manifest digest drift for {}: expected {}, got {}",
                activated.manifest_id, expected_digest, activated.manifest_digest
            )));
        }
        if activated.language_id != manifest.language_id
            || activated.provider_id != manifest.provider_id
            || activated.binary != manifest.binary
            || activated.execution != manifest.execution
        {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "provider activation does not match manifest identity: {}",
                activated.manifest_id
            )));
        }
        providers.push(ActivatedProvider {
            manifest_id: activated.manifest_id.clone(),
            manifest_digest: activated.manifest_digest.clone(),
            language_id: activated.language_id.clone(),
            provider_id: activated.provider_id.clone(),
            binary: activated.binary.clone(),
            execution: activated.execution,
            provider_command_prefix: activated.provider_command_prefix.clone(),
            namespace: manifest.namespace.clone(),
            package_roots: activated.coverage.package_roots.clone(),
            source_extensions: activated.coverage.source_extensions.clone(),
            config_files: activated.coverage.config_files.clone(),
            source_roots: activated.coverage.source_roots.clone(),
            ignored_path_prefixes: activated.coverage.ignored_path_prefixes.clone(),
            search_capabilities: manifest.search_capabilities.clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
            query_pack_descriptor: manifest.query_pack_descriptor.clone(),
            policy: manifest.policy.clone(),
            routes: manifest.routes.clone(),
        });
    }
    Ok(HookRuntime {
        project_root: activation.project_root.clone(),
        providers,
    })
}

impl HookActivation {
    fn validate_protocol(&self) -> Result<(), AgentHookError> {
        expect_field("schemaId", &self.schema_id, HOOK_ACTIVATION_SCHEMA_ID)?;
        expect_field(
            "schemaVersion",
            &self.schema_version,
            HOOK_ACTIVATION_SCHEMA_VERSION,
        )?;
        expect_field("protocolId", &self.protocol_id, HOOK_PROTOCOL_ID)?;
        expect_field(
            "protocolVersion",
            &self.protocol_version,
            HOOK_PROTOCOL_VERSION,
        )?;
        if !matches!(
            self.generated_by.runtime.as_str(),
            "asp" | "agent-semantic-hook"
        ) {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "invalid activation generatedBy.runtime: expected asp or agent-semantic-hook, got {}",
                self.generated_by.runtime
            )));
        }
        if self.providers.is_empty() {
            return Err(AgentHookError::InvalidActivationConfig(
                "activation must include at least one provider".to_string(),
            ));
        }
        Ok(())
    }
}

impl ProviderManifest {
    fn validate_protocol(&self) -> Result<(), AgentHookError> {
        expect_field("schemaId", &self.schema_id, PROVIDER_MANIFEST_SCHEMA_ID)?;
        expect_field(
            "schemaVersion",
            &self.schema_version,
            PROVIDER_MANIFEST_SCHEMA_VERSION,
        )?;
        expect_field("protocolId", &self.protocol_id, HOOK_PROTOCOL_ID)?;
        expect_field(
            "protocolVersion",
            &self.protocol_version,
            HOOK_PROTOCOL_VERSION,
        )?;
        Ok(())
    }
}
