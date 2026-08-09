//! Activation manifest loading and provider/source resolution.

use crate::protocol::{
    AgentHookError, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION,
};

use super::digest::provider_manifest_digest;
use super::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, HookActivation, HookRuntime, ProviderManifest,
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

fn validate_source_snapshot_descriptor(manifest: &ProviderManifest) -> Result<(), AgentHookError> {
    let Some(descriptor) = manifest.search_capabilities.source_snapshot.as_ref() else {
        return Ok(());
    };
    let expected_descriptor_id = format!("{}.source-snapshot", manifest.language_id);
    let valid = descriptor.descriptor_id() == expected_descriptor_id
        && descriptor.descriptor_version() == "1"
        && descriptor.language_id() == manifest.language_id
        && descriptor.packet_schema_id() == "asp.source-snapshot.v1"
        && descriptor.exact_source_packet_schema_id() == "asp.exact-source-query-result.v1"
        && descriptor.canonical_item_selector_schema_id() == "asp.canonical-item-selector.v1"
        && descriptor.source_snapshot_envelope_schema_id()
            == "asp.exact-source-snapshot-envelope.v1"
        && descriptor.derived_artifact_evidence_schema_id()
            == "asp.derived-source-artifact-evidence.v1"
        && descriptor.algorithm() == "blake3-merkle-v1"
        && descriptor.authority() == "live-parser"
        && descriptor.exact_selector_resolution() == "pinned-live-module-graph"
        && descriptor.overlay_mode() == "merkle-delta";
    if valid {
        return Ok(());
    }
    Err(AgentHookError::InvalidActivationConfig(format!(
        "invalid source snapshot descriptor for provider manifest {}",
        manifest.manifest_id
    )))
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
        validate_source_snapshot_descriptor(manifest)?;
        let expected_digest = provider_manifest_digest(manifest)?;
        if activated.manifest_digest != expected_digest {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "provider manifest digest drift for {}: expected {}, got {}",
                activated.manifest_id, expected_digest, activated.manifest_digest
            )));
        }
        let expected_registry_digest = crate::provider_registry::semantic_registry_digest();
        if activated.semantic_registry_digest != expected_registry_digest {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "semantic registry digest drift for {}: expected {}, got {}",
                activated.manifest_id, expected_registry_digest, activated.semantic_registry_digest
            )));
        }
        let expected_routes = crate::provider_registry::materialize_provider_routes(manifest)
            .map_err(|error| {
                AgentHookError::InvalidActivationConfig(format!(
                    "failed to materialize provider routes for {}: {error}",
                    activated.manifest_id
                ))
            })?;
        if activated.routes != expected_routes {
            return Err(AgentHookError::InvalidActivationConfig(format!(
                "provider route drift for {}",
                activated.manifest_id
            )));
        }
        validate_selected_provider_binary(activated)?;
        if activated.language_id != manifest.language_id
            || activated.provider_id != manifest.provider_id
            || activated.execution != manifest.execution
            || activated.search_capabilities != manifest.search_capabilities
            || activated.semantic_facts_descriptor != manifest.semantic_facts_descriptor
            || activated.query_pack_descriptor != manifest.query_pack_descriptor
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
            execution_command_digest: activated.execution_command_digest.clone(),
            namespace: manifest.namespace.clone(),
            package_roots: activated.coverage.package_roots.clone(),
            source_extensions: activated.coverage.source_extensions.clone(),
            config_files: activated.coverage.config_files.clone(),
            search_capabilities: activated.search_capabilities.clone(),
            project_resolution: manifest.project_resolution.clone(),
            document_resolution: manifest.document_resolution.clone(),
            semantic_facts_descriptor: activated.semantic_facts_descriptor.clone(),
            query_pack_descriptor: activated.query_pack_descriptor.clone(),
            policy: manifest.policy.clone(),
            semantic_registry_digest: activated.semantic_registry_digest.clone(),
            routes: activated.routes.clone(),
        });
    }
    Ok(HookRuntime {
        project_root: activation.project_root.clone(),
        rankers: activation.rankers.clone(),
        providers,
        policy_providers: Vec::new(),
    })
}

fn validate_selected_provider_binary(
    activated: &ActivatedProviderConfig,
) -> Result<(), AgentHookError> {
    let selected = std::path::Path::new(&activated.binary);
    let is_logical_basename = selected.components().count() == 1
        && selected.file_name().and_then(|name| name.to_str()) == Some(activated.binary.as_str());
    if !is_logical_basename {
        return Err(AgentHookError::InvalidActivationConfig(format!(
            "provider activation binary must be a logical basename: manifestId={} binary={}",
            activated.manifest_id, activated.binary
        )));
    }
    if !activated.provider_command_prefix.is_empty() {
        return Err(AgentHookError::InvalidActivationConfig(format!(
            "State Home v1 provider activation command prefix must be empty: manifestId={} binary={}",
            activated.manifest_id, activated.binary
        )));
    }
    Ok(())
}

impl HookActivation {
    fn validate_protocol(&self) -> Result<(), AgentHookError> {
        expect_field("schemaId", &self.schema_id, HOOK_ACTIVATION_SCHEMA_ID)?;
        expect_field(
            "schemaVersion",
            &self.schema_version,
            HOOK_ACTIVATION_SCHEMA_VERSION,
        )?;
        expect_field(
            "schemaAuthority",
            &self.schema_authority,
            crate::protocol::CANONICAL_SCHEMA_AUTHORITY,
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
