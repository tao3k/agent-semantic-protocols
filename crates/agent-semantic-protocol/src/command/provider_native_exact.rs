use std::path::Path;

use agent_semantic_content_identity::callable_skeleton_projection::{
    ProviderNativeExactAuthority, ProviderNativeExactProjection,
};
use agent_semantic_hook::RuntimeProfiles;

use super::provider_process::{
    provider_invocation_with_profile, run_provider_command_with_stdin_async,
};

pub(super) struct ProviderNativeExactContext<'a> {
    pub language_id: &'a str,
    pub provider: &'a agent_semantic_hook::ActivatedProvider,
    pub profiles: &'a RuntimeProfiles,
    pub project_root: &'a Path,
}

pub(super) struct ProviderNativeExactRequest<'a> {
    pub owner_path: &'a str,
    pub structural_selector: &'a str,
    pub source_bytes: &'a [u8],
    pub generation_identity_digest: &'a str,
    pub parser_identity_digest: &'a str,
    pub query_pack_digest: &'a str,
}

pub(super) async fn run_provider_native_callable_skeleton(
    context: ProviderNativeExactContext<'_>,
    request: ProviderNativeExactRequest<'_>,
) -> Result<Vec<u8>, String> {
    let source_digest = blake3::hash(request.source_bytes).to_hex().to_string();
    for (field, digest) in [
        (
            "generationIdentityDigest",
            request.generation_identity_digest,
        ),
        ("parserIdentityDigest", request.parser_identity_digest),
        ("queryPackDigest", request.query_pack_digest),
    ] {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "provider-native exact request has invalid {field}: {digest}"
            ));
        }
    }
    let transport = agent_semantic_hook::registered_provider_method_invocation_v1(
        context.language_id,
        context.provider.provider_id.as_str(),
        "query/exact-selector-native-v1",
    )?
    .ok_or_else(|| {
        format!(
            "ProviderRegistry is missing native exact transport: languageId={} providerId={} method=query/exact-selector-native-v1",
            context.language_id, context.provider.provider_id
        )
    })?;
    let registered_binary =
        agent_semantic_hook::registered_provider_binary_v1(context.language_id)?;
    if transport.argv.first().map(String::as_str) != Some(registered_binary.binary()) {
        return Err(format!(
            "ProviderRegistry native exact transport binary drift: languageId={} providerId={}",
            context.language_id, context.provider.provider_id
        ));
    }
    let provider_args = native_exact_provider_args(
        &transport.argv[1..],
        request.structural_selector,
        context.provider.provider_id.as_str(),
    )?;
    let invocation =
        provider_invocation_with_profile(context.profiles, context.provider, &provider_args)?;
    let stdin = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-native-exact-request",
        "schemaVersion": "1",
        "languageId": context.language_id,
        "providerId": context.provider.provider_id.as_str(),
        "structuralSelector": request.structural_selector,
        "ownerPath": request.owner_path,
        "projectionKind": "callable-skeleton",
        "generationIdentityDigest": request.generation_identity_digest,
        "parserIdentityDigest": request.parser_identity_digest,
        "queryPackDigest": request.query_pack_digest,
        "sourceDigest": source_digest,
        "sourceByteLength": request.source_bytes.len() as u64,
        "sourceEncoding": "base64",
        "sourceBytesBase64": super::provider_owner_native::encode_base64(request.source_bytes),
        "transport": "stdin-json"
    }))
    .map_err(|error| format!("encode provider-native exact request: {error}"))?;
    let output = run_provider_command_with_stdin_async(
        context.language_id,
        context.provider,
        &invocation,
        context.project_root,
        stdin,
    )
    .await?;
    if !output.status.success() {
        return Err(format!(
            "provider-native exact query failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let packet: ProviderNativeExactProjection = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| format!("decode typed provider-native exact response: {error}"))?;
    packet.validate_authority(ProviderNativeExactAuthority {
        language_id: context.language_id,
        provider_id: context.provider.provider_id.as_str(),
        owner_path: request.owner_path,
        requested_structural_selector: request.structural_selector,
    })?;
    let canonical_structural_selector = packet.structural_selector.as_str();
    let projection = packet.projection_payload;
    projection
        .validate()
        .map_err(|error| format!("validate callable-skeleton projection: {error}"))?;
    if projection.language_id != context.language_id
        || projection.provider_id != context.provider.provider_id.as_str()
        || projection.root_selector.owner_path != request.owner_path
        || projection.root_selector.selector != canonical_structural_selector
        || projection.root_selector.generation_identity_digest != request.generation_identity_digest
        || projection.root_selector.parser_identity_digest != request.parser_identity_digest
        || projection.root_selector.query_pack_digest != request.query_pack_digest
    {
        return Err("callable-skeleton projection authority mismatch".to_owned());
    }
    serde_json::to_vec(&projection)
        .map_err(|error| format!("encode validated callable-skeleton projection: {error}"))
}

fn native_exact_provider_args(
    registered_args: &[String],
    structural_selector: &str,
    provider_id: &str,
) -> Result<Vec<String>, String> {
    for dynamic_flag in ["--selector", "--projection", "--asp-provider-id"] {
        if registered_args
            .iter()
            .any(|argument| argument == dynamic_flag)
        {
            return Err(format!(
                "ProviderRegistry native exact transport must not encode request identity: flag={dynamic_flag}"
            ));
        }
    }
    let mut args = registered_args.to_vec();
    args.extend([
        "--selector".to_owned(),
        structural_selector.to_owned(),
        "--projection".to_owned(),
        "callable-skeleton".to_owned(),
        "--asp-provider-id".to_owned(),
        provider_id.to_owned(),
    ]);
    Ok(args)
}

#[cfg(test)]
#[path = "../../tests/unit/command/provider_native_exact.rs"]
mod tests;
