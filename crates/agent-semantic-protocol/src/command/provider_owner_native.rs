use std::collections::BTreeSet;
use std::path::Path;

use agent_semantic_client_db::{
    ProviderIncrementalScoped, ProviderOwnerFingerprint, ProviderOwnerMetadata,
    ProviderSelectorProjection,
};
use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use serde::Deserialize;

use super::provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin};

pub(super) struct ProviderOwnerNativeTransportContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) provider: &'a ActivatedProvider,
    pub(super) profiles: &'a RuntimeProfiles,
    pub(super) project_root: &'a Path,
}

pub(super) struct ProviderOwnerNativeRequest<'a> {
    pub(super) scope: &'a ProviderIncrementalScoped,
    pub(super) owner_path: &'a str,
    pub(super) fingerprint: &'a ProviderOwnerFingerprint,
    pub(super) source_bytes: &'a [u8],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProviderNativeOwnerSearchResponse {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) requested_owner_path: String,
    pub(super) requested_projection_mode: String,
    pub(super) source_content_digest: String,
    pub(super) parsed_owner_count: u32,
    pub(super) projection_completeness: String,
    pub(super) projections: Vec<ProviderNativeOwnerProjection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProviderNativeOwnerProjection {
    pub(super) structural_selector: String,
    pub(super) signature: String,
    pub(super) item_kind: String,
    pub(super) item_name: String,
    pub(super) capture_name: String,
    pub(super) source_byte_start: u64,
    pub(super) source_byte_end: u64,
}

pub(super) struct ExpectedOwnerResponse<'a> {
    pub(super) language_id: &'a str,
    pub(super) provider_id: &'a str,
    pub(super) owner_path: &'a str,
    pub(super) projection_mode: &'a str,
    pub(super) content_digest: &'a str,
    pub(super) source_size: u64,
}

pub(super) fn run_provider_owner_native(
    context: ProviderOwnerNativeTransportContext<'_>,
    request: ProviderOwnerNativeRequest<'_>,
) -> Result<Vec<ProviderSelectorProjection>, String> {
    validate_transport_identity(&context, &request)?;
    let stdin = encode_request(&context, &request)?;
    let transport = agent_semantic_hook::registered_provider_method_invocation_v1(
        context.language_id,
        context.provider.provider_id.as_str(),
        "search/owner-native",
    )?
    .ok_or_else(|| {
        format!(
            "ProviderRegistry is missing native owner transport: languageId={} providerId={} method=search/owner-native",
            context.language_id, context.provider.provider_id
        )
    })?;
    let registered_binary =
        agent_semantic_hook::registered_provider_binary_v1(context.language_id)?;
    let expected_binary = registered_binary.binary();
    let transport_binary = transport.argv.first().ok_or_else(|| {
        format!(
            "ProviderRegistry native owner transport argv is empty: languageId={} providerId={}",
            context.language_id, context.provider.provider_id
        )
    })?;
    if transport_binary != expected_binary {
        return Err(format!(
            "ProviderRegistry native owner transport binary drift: languageId={} providerId={} expected={} actual={transport_binary}",
            context.language_id, context.provider.provider_id, expected_binary
        ));
    }
    let invocation =
        provider_invocation_with_profile(context.profiles, context.provider, &transport.argv[1..])?;
    let output = run_provider_command_with_stdin(
        context.language_id,
        context.provider,
        &invocation,
        context.project_root,
        stdin,
    )?;
    if !output.status.success() {
        return Err(format!(
            "provider owner-search-stdin failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let response: ProviderNativeOwnerSearchResponse =
        serde_json::from_slice(output.stdout.as_ref())
            .map_err(|error| format!("invalid provider owner-search response JSON: {error}"))?;
    validate_provider_owner_response(
        response,
        ExpectedOwnerResponse {
            language_id: context.language_id,
            provider_id: request.scope.provider_id.as_str(),
            owner_path: request.owner_path,
            projection_mode: "complete-owner",
            content_digest: request.fingerprint.content_digest.as_str(),
            source_size: request.source_bytes.len() as u64,
        },
    )
}

fn validate_transport_identity(
    context: &ProviderOwnerNativeTransportContext<'_>,
    request: &ProviderOwnerNativeRequest<'_>,
) -> Result<(), String> {
    if context.provider.provider_id.as_str() != request.scope.provider_id
        || context.provider.language_id != context.language_id
        || request.source_bytes.len() as u64 != request.fingerprint.metadata.size_bytes
    {
        return Err("activated provider owner-native request identity drift".into());
    }
    Ok(())
}

fn encode_request(
    context: &ProviderOwnerNativeTransportContext<'_>,
    request: &ProviderOwnerNativeRequest<'_>,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-request",
        "schemaVersion": "1",
        "languageId": context.language_id,
        "providerId": request.scope.provider_id,
        "workspaceIdentity": request.scope.workspace_identity,
        "providerWorkspaceIdentityDigest": request.scope.provider_workspace_identity_digest,
        "ownerPath": request.owner_path,
        "sourceFingerprint": {
            "fileIdentity": request.fingerprint.metadata.file_identity,
            "sizeBytes": request.fingerprint.metadata.size_bytes,
            "modifiedUnixNanos": request.fingerprint.metadata.modified_unix_nanos,
            "changeTimeUnixNanos": request.fingerprint.metadata.change_time_unix_nanos,
            "contentDigest": request.fingerprint.content_digest,
        },
        "sourceEncoding": "base64",
        "sourceBytesBase64": encode_base64(request.source_bytes),
        "projectionMode": "complete-owner",
        "transport": "stdin-json",
    }))
    .map_err(|error| format!("failed to encode provider owner-search request: {error}"))
}

pub(super) fn validate_provider_owner_response(
    response: ProviderNativeOwnerSearchResponse,
    expected: ExpectedOwnerResponse<'_>,
) -> Result<Vec<ProviderSelectorProjection>, String> {
    if response.schema_id != "agent.semantic-protocols.provider-native-owner-search-response"
        || response.schema_version != "1"
        || response.language_id != expected.language_id
        || response.provider_id != expected.provider_id
        || response.requested_owner_path != expected.owner_path
        || response.requested_projection_mode != expected.projection_mode
        || response.source_content_digest != expected.content_digest
        || response.parsed_owner_count != 1
        || response.projection_completeness != "complete-owner"
    {
        return Err("provider owner-search response identity or completeness drift".into());
    }
    let selector_prefix = format!("{}://{}#item/", expected.language_id, expected.owner_path);
    let mut selectors = BTreeSet::new();
    response
        .projections
        .into_iter()
        .map(|projection| {
            validate_projection(
                &projection,
                selector_prefix.as_str(),
                expected.source_size,
                &mut selectors,
            )?;
            Ok(ProviderSelectorProjection {
                structural_selector: projection.structural_selector,
                capture_name: projection.capture_name,
                signature: projection.signature,
                item_kind: projection.item_kind,
                item_name: projection.item_name,
                source_byte_start: projection.source_byte_start,
                source_byte_end: projection.source_byte_end,
            })
        })
        .collect()
}

fn validate_projection(
    projection: &ProviderNativeOwnerProjection,
    selector_prefix: &str,
    source_size: u64,
    selectors: &mut BTreeSet<String>,
) -> Result<(), String> {
    if projection.structural_selector.is_empty()
        || !projection.structural_selector.starts_with(selector_prefix)
        || !selectors.insert(projection.structural_selector.clone())
        || projection.signature.is_empty()
        || projection.item_kind.is_empty()
        || projection.item_name.is_empty()
        || projection.capture_name.is_empty()
        || projection.source_byte_start >= projection.source_byte_end
        || projection.source_byte_end > source_size
    {
        return Err(format!(
            "provider owner-search projection is invalid: selector={} span={}..{} sourceSize={source_size}",
            projection.structural_selector,
            projection.source_byte_start,
            projection.source_byte_end
        ));
    }
    Ok(())
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    bytes.chunks(3).for_each(|chunk| {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(ALPHABET[(first >> 2) as usize] as char);
        encoded.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    });
    encoded
}

#[cfg(unix)]
pub(super) fn provider_owner_metadata(path: &Path) -> Result<ProviderOwnerMetadata, String> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect owner metadata {}: {error}",
            path.display()
        )
    })?;
    Ok(ProviderOwnerMetadata {
        file_identity: format!("unix:{}:{}", metadata.dev(), metadata.ino()),
        size_bytes: metadata.len(),
        modified_unix_nanos: metadata
            .mtime()
            .saturating_mul(1_000_000_000)
            .saturating_add(metadata.mtime_nsec()),
        change_time_unix_nanos: metadata
            .ctime()
            .saturating_mul(1_000_000_000)
            .saturating_add(metadata.ctime_nsec()),
    })
}

#[cfg(not(unix))]
pub(super) fn provider_owner_metadata(path: &Path) -> Result<ProviderOwnerMetadata, String> {
    use std::time::UNIX_EPOCH;

    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect owner metadata {}: {error}",
            path.display()
        )
    })?;
    let modified_unix_nanos = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos().try_into().unwrap_or(i64::MAX))
        .unwrap_or_default();
    Ok(ProviderOwnerMetadata {
        file_identity: format!("path:{}", path.display()),
        size_bytes: metadata.len(),
        modified_unix_nanos,
        change_time_unix_nanos: modified_unix_nanos,
    })
}
