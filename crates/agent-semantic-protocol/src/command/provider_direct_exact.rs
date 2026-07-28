//! One-owner, in-memory provider execution for canonical exact selectors.

use std::path::Path;
use std::time::Instant;

use agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1;
use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};

use super::provider_execution::provider_process_args;
use super::provider_process::{provider_invocations, run_provider_command_with_stdin};
use super::provider_selector::{
    provider_owned_structural_owner_path, provider_owned_structural_selector,
};

/// Complete direct exact-query execution context.
pub(super) struct DirectExactQueryContextV1<'a> {
    pub(super) language_id: &'a str,
    pub(super) provider_args: &'a [String],
    pub(super) project_root: &'a Path,
    pub(super) provider: &'a ActivatedProvider,
    pub(super) runtime_profiles: &'a RuntimeProfiles,
    pub(super) started: Instant,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactRequestV1<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    language_id: &'a str,
    provider_id: &'a str,
    structural_selector: &'a str,
    owner_path: &'a str,
    source_digest: &'a str,
    source_byte_length: usize,
    source_encoding: &'static str,
    source_bytes_base64: String,
    transport: &'static str,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactProjectionV1 {
    schema_id: String,
    schema_version: String,
    language_id: String,
    provider_id: String,
    owner_path: String,
    requested_structural_selector: String,
    structural_selector: String,
    projection_mode: ExactProjectionModeV1,
    projection_text: String,
    source_content_digest: String,
    source_byte_start: u64,
    source_byte_end: u64,
}

type ActiveExactFixtureResidentV1 = agent_semantic_search::ExactSelectorFixtureResidentV1<
    agent_semantic_search::ExactSelectorFixtureFileBackendV1,
>;

fn active_exact_fixture_resident_v1(
    input: &agent_semantic_hook::ActiveAspArtifactInput,
) -> Result<std::sync::Arc<ActiveExactFixtureResidentV1>, String> {
    static RESIDENTS: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<ActiveExactFixtureResidentV1>>,
        >,
    > = std::sync::OnceLock::new();
    let residents = RESIDENTS.get_or_init(Default::default);
    let mut residents = residents
        .lock()
        .map_err(|_| "active exact fixture resident registry is poisoned".to_owned())?;
    if let Some(resident) = residents.get(&input.artifact_digest) {
        return Ok(std::sync::Arc::clone(resident));
    }
    let backend = agent_semantic_search::active_exact_selector_fixture::
        exact_selector_fixture_backend_from_active_artifact_v1(input)?;
    let resident = std::sync::Arc::new(
        agent_semantic_search::ExactSelectorFixtureResidentV1::new(backend),
    );
    residents.insert(input.artifact_digest.clone(), std::sync::Arc::clone(&resident));
    Ok(resident)
}

fn requested_projection_mode_v1(
    provider_args: &[String],
) -> agent_semantic_content_identity::ExactSelectorProjectionModeV1 {
    if provider_args.iter().any(|arg| arg == "--verbatim") {
        agent_semantic_content_identity::ExactSelectorProjectionModeV1::Verbatim
    } else if provider_args.iter().any(|arg| arg == "--names-only") {
        agent_semantic_content_identity::ExactSelectorProjectionModeV1::Names
    } else if provider_args.iter().any(|arg| arg == "--code") {
        agent_semantic_content_identity::ExactSelectorProjectionModeV1::Code
    } else {
        agent_semantic_content_identity::ExactSelectorProjectionModeV1::Skeleton
    }
}

pub(super) fn try_run_active_fixture_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    activation_path: &Path,
    started: Instant,
) -> Result<bool, String> {
    if provider_args.iter().any(|arg| arg == "--json") {
        return Ok(false);
    }
    let Some(owner_path) = provider_owned_structural_owner_path(language_id, provider_args) else {
        return Ok(false);
    };
    let Some(structural_selector) =
        provider_owned_structural_selector(language_id, provider_args)
    else {
        return Ok(false);
    };
    let receipt_path = agent_semantic_hook::active_asp_artifact_receipt_path(activation_path)?;
    if !receipt_path.is_file() {
        return Ok(false);
    }
    let current_asp = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current ASP binary: {error}"))?;
    let receipt =
        match agent_semantic_hook::verify_active_asp_artifact_receipt(activation_path, &[&current_asp])
        {
            Ok(receipt) => receipt,
            Err(_) => return Ok(false),
        };
    let input =
        match agent_semantic_search::active_exact_selector_fixture::
            exact_selector_fixture_active_artifact_input_v1(&receipt)
        {
            Ok(input) => input,
            Err(error) if error.contains("reasonKind=active-fixture-missing") => return Ok(false),
            Err(error) => return Err(error),
        };
    let resident = active_exact_fixture_resident_v1(&input)?;
    let Some(projection) = resident.resolve(structural_selector)? else {
        return Ok(false);
    };
    let canonical_workspace = project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            project_root.display()
        )
    })?;
    let canonical_owner = canonical_exact_owner(&canonical_workspace, owner_path)?;
    let source = std::fs::read(&canonical_owner).map_err(|error| {
        format!("failed to read exact-selector owner bytes: ownerPath={owner_path} error={error}")
    })?;
    if !projection.matches_live_owner_v1(
        owner_path,
        &source,
        requested_projection_mode_v1(provider_args),
    ) {
        return Ok(false);
    }
    std::io::Write::write_all(&mut std::io::stdout().lock(), projection.as_bytes())
        .map_err(|error| format!("failed to write resident exact-selector projection: {error}"))?;
    trace("resident-fixture-hit", started);
    Ok(true)
}

pub(super) fn run_catalog_direct_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: Instant,
) -> Result<(), String> {
    let provider =
        super::global_provider_catalog::global_provider_for_language_v1(language_id)?;
    let owner_path = provider_owned_structural_owner_path(language_id, provider_args).ok_or_else(
        || "catalog exact query is missing an exact owner path".to_owned(),
    )?;
    let structural_selector =
        provider_owned_structural_selector(language_id, provider_args).ok_or_else(|| {
            "catalog exact query is missing an exact structural selector".to_owned()
        })?;
    let canonical_workspace = project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            project_root.display()
        )
    })?;
    let canonical_owner = canonical_exact_owner(&canonical_workspace, owner_path)?;
    let source = std::fs::read(&canonical_owner).map_err(|error| {
        format!("failed to read exact-selector owner bytes: ownerPath={owner_path} error={error}")
    })?;
    trace("catalog-owner-read", started);
    let source_content_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&source)
            .as_str()
            .to_owned();
    let request = ProviderNativeExactRequestV1 {
        schema_id: "agent.semantic-protocols.provider-native-exact-request",
        schema_version: "1",
        language_id,
        provider_id: &provider.provider_id,
        structural_selector,
        owner_path,
        source_digest: &source_content_digest,
        source_byte_length: source.len(),
        source_encoding: "base64",
        source_bytes_base64: encode_base64(&source),
        transport: "stdin-json",
    };
    let request = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode catalog exact-selector request: {error}"))?;
    let template = agent_semantic_hook::registered_provider_method_invocation_v1(
        language_id,
        &provider.provider_id,
        "query/exact-selector-native-v1",
    )?
    .ok_or_else(|| {
        format!(
            "Global provider catalog has no exact-selector method: language={} provider={}",
            language_id, provider.provider_id
        )
    })?;
    let mut invocation = template.argv;
    let program = invocation.first_mut().ok_or_else(|| {
        format!(
            "Global provider catalog exact method has empty argv: language={language_id}"
        )
    })?;
    *program = provider.materialized_path.clone();
    invocation.push("--json".to_owned());
    invocation.extend(
        provider_args
            .iter()
            .filter(|arg| {
                matches!(
                    arg.as_str(),
                    "--names-only" | "--code" | "--verbatim" | "--skeleton"
                )
            })
            .cloned(),
    );
    if !invocation
        .windows(2)
        .any(|pair| pair[0] == "--asp-provider-id")
    {
        invocation.extend([
            "--asp-provider-id".to_owned(),
            provider.provider_id.clone(),
        ]);
    }
    let output = super::provider_process::run_catalog_provider_command_with_stdin(
        language_id,
        &provider.provider_id,
        &provider.execution_command_digest,
        &invocation,
        project_root,
        request,
    )?;
    trace("catalog-provider-complete", started);
    if !output.status.success() {
        return Err(format!(
            "catalog exact-selector provider failed: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(output.stderr.as_ref())
        ));
    }
    let native: ProviderNativeExactProjectionV1 = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| format!("failed to decode catalog exact-selector output: {error}"))?;
    validate_projection(
        &native,
        language_id,
        &provider.provider_id,
        provider_args,
        owner_path,
        structural_selector,
        &source_content_digest,
        &source,
    )?;
    if provider_args.iter().any(|arg| arg == "--json") {
        std::io::Write::write_all(&mut std::io::stdout().lock(), output.stdout.as_ref())
            .map_err(|error| format!("failed to write catalog exact-selector packet: {error}"))
    } else {
        std::io::Write::write_all(
            &mut std::io::stdout().lock(),
            native.projection_text.as_bytes(),
        )
        .map_err(|error| format!("failed to write catalog exact-selector projection: {error}"))
    }
}

/// Run one canonical exact selector from one live workspace-contained owner.
pub(super) fn run_direct_exact_query(context: DirectExactQueryContextV1<'_>) -> Result<(), String> {
    let owner_path =
        provider_owned_structural_owner_path(context.language_id, context.provider_args)
            .ok_or_else(|| {
                "provider-owned structural query is missing an exact owner path".to_string()
            })?;
    let structural_selector =
        provider_owned_structural_selector(context.language_id, context.provider_args).ok_or_else(
            || "provider-owned structural query is missing an exact selector".to_string(),
        )?;
    let canonical_workspace = context.project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            context.project_root.display()
        )
    })?;
    let canonical_owner = canonical_exact_owner(&canonical_workspace, owner_path)?;
    let source = std::fs::read(&canonical_owner).map_err(|error| {
        format!("failed to read exact-selector owner bytes: ownerPath={owner_path} error={error}")
    })?;
    trace("direct-owner-read", context.started);
    let source_content_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&source)
            .as_str()
            .to_owned();
    let request = ProviderNativeExactRequestV1 {
        schema_id: "agent.semantic-protocols.provider-native-exact-request",
        schema_version: "1",
        language_id: context.language_id,
        provider_id: context.provider.provider_id.as_str(),
        structural_selector,
        owner_path,
        source_digest: &source_content_digest,
        source_byte_length: source.len(),
        source_encoding: "base64",
        source_bytes_base64: encode_base64(&source),
        transport: "stdin-json",
    };
    let request = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode exact-selector stdin request: {error}"))?;
    let mut provider_argv = provider_process_args(context.provider_args);
    provider_argv.extend([
        "--json".to_string(),
        "--asp-provider-id".to_string(),
        context.provider.provider_id.to_string(),
        "--asp-exact-request-stdin".to_string(),
    ]);
    let invocations = provider_invocations(
        context.provider,
        &provider_argv,
        context.project_root,
        context.runtime_profiles,
    )?;
    let [invocation] = invocations.as_slice() else {
        return Err(format!(
            "direct exact-selector projection requires one provider invocation; invocations={}",
            invocations.len()
        ));
    };
    let output = run_provider_command_with_stdin(
        context.language_id,
        context.provider,
        invocation,
        context.project_root,
        request,
    )?;
    trace("direct-provider-complete", context.started);
    if !output.status.success() {
        return Err(format!(
            "direct exact-selector provider failed: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(output.stderr.as_ref())
        ));
    }
    let native: ProviderNativeExactProjectionV1 = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| {
            format!("failed to decode direct exact-selector provider output: {error}")
        })?;
    validate_projection(
        &native,
        context.language_id,
        context.provider.provider_id.as_str(),
        context.provider_args,
        owner_path,
        structural_selector,
        &source_content_digest,
        &source,
    )?;
    if context.provider_args.iter().any(|arg| arg == "--json") {
        std::io::Write::write_all(&mut std::io::stdout().lock(), output.stdout.as_ref())
            .map_err(|error| format!("failed to write direct exact-selector packet: {error}"))
    } else {
        std::io::Write::write_all(
            &mut std::io::stdout().lock(),
            native.projection_text.as_bytes(),
        )
        .map_err(|error| format!("failed to write direct exact-selector projection: {error}"))
    }
}

fn canonical_exact_owner(workspace: &Path, owner_path: &str) -> Result<std::path::PathBuf, String> {
    let owner = Path::new(owner_path);
    if owner.is_absolute()
        || owner.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "exact-selector owner is not a canonical workspace-relative path: {owner_path}"
        ));
    }
    let canonical_owner = workspace.join(owner).canonicalize().map_err(|error| {
        format!(
            "failed to resolve exact-selector owner inside workspace: ownerPath={owner_path} error={error}"
        )
    })?;
    if !canonical_owner.starts_with(workspace) || !canonical_owner.is_file() {
        return Err(format!(
            "exact-selector owner escaped or is not a file in the provider workspace: ownerPath={owner_path}"
        ));
    }
    Ok(canonical_owner)
}

fn validate_projection(
    native: &ProviderNativeExactProjectionV1,
    language_id: &str,
    provider_id: &str,
    provider_args: &[String],
    owner_path: &str,
    structural_selector: &str,
    source_content_digest: &str,
    source: &[u8],
) -> Result<(), String> {
    let projection_mode = if provider_args.iter().any(|arg| arg == "--names-only") {
        ExactProjectionModeV1::Names
    } else {
        ExactProjectionModeV1::Code
    };
    validate_projection_field(
        "schemaId",
        &native.schema_id,
        "agent.semantic-protocols.provider-native-exact-projection",
    )?;
    validate_projection_field("schemaVersion", &native.schema_version, "1")?;
    validate_projection_field("languageId", &native.language_id, language_id)?;
    validate_projection_field("providerId", &native.provider_id, provider_id)?;
    validate_projection_field("ownerPath", &native.owner_path, owner_path)?;
    validate_projection_field(
        "requestedStructuralSelector",
        &native.requested_structural_selector,
        structural_selector,
    )?;
    if native.structural_selector.is_empty() {
        return Err("direct exact-selector canonical structuralSelector is empty".to_string());
    }
    if native.projection_mode != projection_mode {
        return Err(format!(
            "direct exact-selector projectionMode mismatch: expected={projection_mode:?} actual={:?}",
            native.projection_mode
        ));
    }
    validate_projection_field(
        "sourceContentDigest",
        &native.source_content_digest,
        source_content_digest,
    )?;
    if native.source_byte_start > native.source_byte_end {
        return Err(format!(
            "direct exact-selector byte span is reversed: start={} end={}",
            native.source_byte_start, native.source_byte_end
        ));
    }
    if native.source_byte_end > source.len() as u64 {
        return Err(format!(
            "direct exact-selector byte span exceeds owner bytes: end={} sourceByteLength={}",
            native.source_byte_end,
            source.len()
        ));
    }
    if projection_mode == ExactProjectionModeV1::Code {
        let start = usize::try_from(native.source_byte_start)
            .map_err(|_| "direct exact-selector byte start overflow".to_string())?;
        let end = usize::try_from(native.source_byte_end)
            .map_err(|_| "direct exact-selector byte end overflow".to_string())?;
        if native.projection_text.as_bytes() != &source[start..end] {
            return Err(
                "direct exact-selector projection is not the parser-owned source byte slice"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn validate_projection_field(field: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "direct exact-selector {field} mismatch: expected={expected:?} actual={actual:?}"
    ))
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_EXACT_QUERY_TRACE").is_some() {
        eprintln!(
            "[exact-query-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}
