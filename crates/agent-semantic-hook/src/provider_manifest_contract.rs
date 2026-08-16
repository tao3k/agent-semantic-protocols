use super::ProviderManifest;

pub fn validate_provider_manifest_contract(manifest: &ProviderManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest.language_id.is_empty() {
        errors.push("provider manifest languageId must be non-empty".to_string());
    }
    if manifest.provider_id.is_empty() {
        errors.push("provider manifest providerId must be non-empty".to_string());
    }

    if let Err(error) =
        crate::protocol_activation::protocol_activation_runtime::validate_runtime_contract(manifest)
    {
        errors.push(error.to_string());
    }

    if let Err(error) =
        crate::protocol_activation::provider_query_pack::validate_query_pack_descriptor(manifest)
    {
        errors.push(error.to_string());
    }
    if let Err(error) =
        crate::protocol_activation::provider_query_pack::validate_semantic_facts_descriptor(
            manifest,
        )
    {
        errors.push(error.to_string());
    }
    if let Err(error) = validate_source_snapshot_capability(
        manifest.language_id.as_str(),
        &manifest.search_capabilities,
    ) {
        errors.push(error);
    }
    if let Err(error) = validate_project_resolution_descriptor(manifest) {
        errors.push(error);
    }
    if let Some(descriptor) = manifest.language_projection()
        && let Err(error) = validate_language_projection_descriptor(descriptor)
    {
        errors.push(error);
    }

    errors
}

fn validate_language_projection_descriptor(
    descriptor: &crate::protocol_activation::protocol_activation_manifest::ProviderLanguageProjectionDescriptor,
) -> Result<(), String> {
    if descriptor.schema_id() != "agent.semantic-protocols.provider-language-projection-descriptor"
        || descriptor.schema_version() != "1"
        || descriptor.command_binding() != "projection-batch-stdin"
        || descriptor.transport() != "framed-stdin-v1"
        || descriptor.request_schema()
            != "https://schemas.agent-semantic-protocols.dev/provider-language-projection-batch-request.v1.schema.json"
        || descriptor.response_schema()
            != "https://schemas.agent-semantic-protocols.dev/provider-language-projection-batch-response.v1.schema.json"
        || descriptor.identity_schema()
            != "https://schemas.agent-semantic-protocols.dev/canonical-language-item-identity.v1.schema.json"
    {
        return Err("provider languageProjection descriptor contract mismatch".to_string());
    }
    Ok(())
}

fn validate_project_resolution_descriptor(manifest: &ProviderManifest) -> Result<(), String> {
    let Some(descriptor) = manifest.project_resolution() else {
        return validate_document_resolution_descriptor(manifest);
    };
    if manifest.document_resolution().is_some() {
        return Err(format!(
            "provider {} must declare exactly one of projectResolution or documentResolution",
            manifest.provider_id()
        ));
    }
    if descriptor.schema_id != "agent.semantic-protocols.provider-project-resolution-descriptor"
        || descriptor.schema_version != "1"
    {
        return Err(format!(
            "provider {} projectResolution schema must be agent.semantic-protocols.provider-project-resolution-descriptor v1",
            manifest.provider_id()
        ));
    }
    if descriptor.capability_id != "project-resolution" {
        return Err(format!(
            "provider {} projectResolution capabilityId must be project-resolution",
            manifest.provider_id()
        ));
    }
    if descriptor.entry_markers.is_empty()
        || descriptor
            .entry_markers
            .iter()
            .any(|marker| marker.is_empty())
    {
        return Err(format!(
            "provider {} projectResolution entryMarkers must be non-empty",
            manifest.provider_id()
        ));
    }
    if descriptor.parser_id.is_empty() || descriptor.command_binding != "project-resolution-stdin" {
        return Err(format!(
            "provider {} projectResolution requires a non-empty parserId and commandBinding=project-resolution-stdin",
            manifest.provider_id()
        ));
    }
    for (field, actual, expected) in [
        (
            "requestSchema",
            descriptor.request_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/provider-project-resolution-request.v1.schema.json",
        ),
        (
            "responseSchema",
            descriptor.response_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/provider-project-resolution-response.v1.schema.json",
        ),
        (
            "packageGraphSchema",
            descriptor.package_graph_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/language-package-graph.v1.schema.json",
        ),
        (
            "projectResolutionSchema",
            descriptor.project_resolution_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/project-resolution.v1.schema.json",
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "provider {} projectResolution {field} must be {expected}",
                manifest.provider_id()
            ));
        }
    }
    Ok(())
}

fn validate_document_resolution_descriptor(manifest: &ProviderManifest) -> Result<(), String> {
    let descriptor = manifest.document_resolution().ok_or_else(|| {
        format!(
            "provider {} must declare exactly one of projectResolution or documentResolution",
            manifest.provider_id()
        )
    })?;
    if descriptor.schema_id != "agent.semantic-protocols.provider-document-resolution-descriptor"
        || descriptor.schema_version != "1"
        || descriptor.capability_id != "document-resolution"
        || !descriptor.supports_git_candidates
        || descriptor.parser_id.is_empty()
        || descriptor.extensions.is_empty()
        || descriptor
            .extensions
            .iter()
            .any(|extension| !extension.starts_with('.'))
    {
        return Err(format!(
            "provider {} has an invalid documentResolution descriptor",
            manifest.provider_id()
        ));
    }
    Ok(())
}

pub(super) fn validate_source_snapshot_capability(
    language_id: &str,
    search_capabilities: &crate::protocol_activation::protocol_activation_manifest::ProviderSearchCapabilities,
) -> Result<(), String> {
    let descriptor = search_capabilities
        .source_snapshot
        .as_ref()
        .ok_or_else(|| {
            format!(
                "provider `{language_id}` is missing required searchCapabilities.sourceSnapshot descriptor"
            )
        })?;
    let descriptor_id = (!descriptor.descriptor_id().is_empty())
        .then_some(descriptor.descriptor_id())
        .ok_or_else(|| format!("provider `{language_id}` source snapshot descriptorId is empty"))?;
    for (field, actual, expected) in [
        (
            "descriptorVersion",
            descriptor.descriptor_version(),
            "1",
        ),
        ("languageId", descriptor.language_id(), language_id),
        (
            "packetSchemaId",
            descriptor.packet_schema_id(),
            "asp.source-snapshot.v1",
        ),
        (
            "exactSourcePacketSchemaId",
            descriptor.exact_source_packet_schema_id(),
            "asp.exact-source-query-result.v1",
        ),
        (
            "canonicalItemSelectorSchemaId",
            descriptor.canonical_item_selector_schema_id(),
            agent_semantic_content_identity::canonical_item_identity::CANONICAL_ITEM_SELECTOR_SCHEMA_ID,
        ),
        (
            "sourceSnapshotEnvelopeSchemaId",
            descriptor.source_snapshot_envelope_schema_id(),
            "asp.exact-source-snapshot-envelope.v1",
        ),
        (
            "derivedArtifactEvidenceSchemaId",
            descriptor.derived_artifact_evidence_schema_id(),
            "asp.derived-source-artifact-evidence.v1",
        ),
        (
            "algorithm",
            descriptor.algorithm(),
            "blake3-merkle-v1",
        ),
        ("authority", descriptor.authority(), "live-parser"),
        (
            "exactSelectorResolution",
            descriptor.exact_selector_resolution(),
            "pinned-live-module-graph",
        ),
        (
            "overlayMode",
            descriptor.overlay_mode(),
            "merkle-delta",
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "provider `{language_id}` source snapshot descriptor `{descriptor_id}` requires {field}={expected}, got {actual}"
            ));
        }
    }
    Ok(())
}
