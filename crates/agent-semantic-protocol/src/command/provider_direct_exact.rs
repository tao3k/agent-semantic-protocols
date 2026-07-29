//! One-owner, in-memory provider execution for canonical exact selectors.

use std::path::Path;
use std::time::Instant;

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};

use super::provider_execution::provider_process_args;
use super::provider_process::{provider_invocations, run_provider_command_with_stdin};
use super::provider_selector::{
    provider_owned_structural_owner_path, provider_owned_structural_selector,
};

/// Complete direct exact-query execution context.
pub(super) struct DirectExactQueryContext<'a> {
    pub(super) language_id: &'a str,
    pub(super) provider_args: &'a [String],
    pub(super) project_root: &'a Path,
    pub(super) provider: &'a ActivatedProvider,
    pub(super) runtime_profiles: &'a RuntimeProfiles,
    pub(super) started: Instant,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactRequest<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    language_id: &'a str,
    provider_id: &'a str,
    structural_selector: &'a str,
    owner_path: &'a str,
    projection_kind: &'a str,
    generation_identity_digest: &'a str,
    parser_identity_digest: &'a str,
    query_pack_digest: &'a str,
    source_digest: &'a str,
    source_byte_length: usize,
    source_encoding: &'static str,
    source_bytes_base64: String,
    transport: &'static str,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactProjection {
    schema_id: String,
    schema_version: String,
    language_id: String,
    provider_id: String,
    owner_path: String,
    requested_structural_selector: String,
    structural_selector: String,
    projection_mode: ProviderNativeExactProjectionMode,
    projection_text: Option<String>,
    projection_payload: Option<
        agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonProjectionV1,
    >,
    source_content_digest: String,
    source_byte_start: u64,
    source_byte_end: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactResolution {
    schema_id: String,
    schema_version: String,
    language_id: String,
    provider_id: String,
    owner_path: String,
    requested_structural_selector: String,
    resolution_state: String,
    reason_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_digest: Option<String>,
    item_kind: String,
    item_name: String,
    #[serde(default)]
    candidates: Vec<String>,
    #[serde(default)]
    actual_kinds: Vec<String>,
    recommended_next: ProviderNativeExactRecommendedNext,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderNativeExactRecommendedNext {
    command: String,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ProviderNativeExactResponse {
    Resolution(ProviderNativeExactResolution),
    Projection(ProviderNativeExactProjection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ProviderNativeExactProjectionMode {
    Source,
    CallableSkeleton,
}

pub(super) fn try_run_resident_turso_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: Instant,
) -> Result<bool, String> {
    let projection_kind = direct_projection_kind(provider_args)?;
    if provider_args.iter().any(|arg| arg == "--json") {
        return Ok(false);
    }
    let Some(owner_path) = provider_owned_structural_owner_path(language_id, provider_args) else {
        return Ok(false);
    };
    let Some(structural_selector) = provider_owned_structural_selector(language_id, provider_args)
    else {
        return Ok(false);
    };
    let provider = super::global_provider_catalog::global_provider_for_language(language_id)?;
    let canonical_project_root = project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            project_root.display()
        )
    })?;
    let Some(canonical_item_selector) = resident_canonical_item_selector(structural_selector)?
    else {
        trace("resident-turso-descendant-provider-fallback", started);
        return Ok(false);
    };
    let session = match super::workspace_db_resident::session(project_root) {
        Ok(session) => session,
        Err(error) if error.is_unavailable() => {
            trace("resident-turso-service-unavailable", started);
            return Ok(false);
        }
        Err(error) => return Err(error.to_string()),
    };
    let read = super::workspace_db_runtime::block_on(session.read_resident_selector(
        &agent_semantic_client_db::TursoResidentSelectorQuery {
            project_root: canonical_project_root.display().to_string(),
            schema_id: agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID.to_owned(),
            schema_version:
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION.to_owned(),
            provider_id: provider.provider_id.clone(),
            parser_identity_digest: provider.exact_parser_identity_digest.clone(),
            query_pack_digest: provider.exact_query_pack_identity_digest.clone(),
            canonical_item_selector,
        },
    ))??;
    let Some(read) = read else {
        trace("resident-turso-generation-missing", started);
        return Ok(false);
    };
    let candidate = read
        .candidates
        .iter()
        .find(|candidate| candidate.owner_path == owner_path)
        .or_else(|| match read.candidates.as_slice() {
            [candidate] => Some(candidate),
            _ => None,
        });
    let Some(candidate) = candidate else {
        trace("resident-turso-selector-miss", started);
        return Ok(false);
    };
    let Some(projection) = candidate.projection.as_ref() else {
        trace("resident-turso-projection-missing", started);
        return Ok(false);
    };
    if projection.structural_selector != structural_selector {
        return Err("resident Turso projection selector identity mismatch".to_owned());
    }
    let Some(canonical_owner) =
        canonical_exact_owner(&canonical_project_root, &candidate.owner_path)?
    else {
        trace("resident-turso-owner-missing", started);
        return Ok(false);
    };
    let Some(source) = read_exact_owner(&canonical_owner, &candidate.owner_path)? else {
        trace("resident-turso-owner-disappeared", started);
        return Ok(false);
    };
    let live_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&source);
    if live_digest.as_str() != candidate.owner_content_digest {
        trace("resident-turso-live-owner-mismatch", started);
        return Ok(false);
    }
    let start = usize::try_from(projection.source_byte_start)
        .map_err(|_| "resident Turso projection start exceeds address space".to_owned())?;
    let end = usize::try_from(projection.source_byte_end)
        .map_err(|_| "resident Turso projection end exceeds address space".to_owned())?;
    let payload = match projection_kind {
        "source" => source.get(start..end).ok_or_else(|| {
            format!(
                "resident Turso projection byte range is invalid: start={start} end={end} sourceBytes={}",
                source.len()
            )
        })?,
        "callable-skeleton" => projection.signature.as_bytes(),
        _ => unreachable!("projection kind is validated before resident lookup"),
    };
    std::io::Write::write_all(&mut std::io::stdout().lock(), payload)
        .map_err(|error| format!("failed to write resident Turso exact projection: {error}"))?;
    trace("resident-turso-hit", started);
    Ok(true)
}

pub(super) fn resident_canonical_item_selector(
    structural_selector: &str,
) -> Result<
    Option<agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector>,
    String,
> {
    let canonical_item_selector = agent_semantic_content_identity::canonical_item_identity::
        CanonicalItemSelector::parse_root_or_exact_descendant(structural_selector)
        .map_err(|error| format!("invalid exact-selector canonical identity: {error}"))?;
    Ok(
        (canonical_item_selector.structural_selector() == structural_selector)
            .then_some(canonical_item_selector),
    )
}

pub(super) fn run_catalog_direct_exact_query(
    language_id: &str,
    provider_args: &[String],
    project_root: &Path,
    started: Instant,
) -> Result<(), String> {
    let provider = super::global_provider_catalog::global_provider_for_language(language_id)?;
    let owner_path = provider_owned_structural_owner_path(language_id, provider_args)
        .ok_or_else(|| "catalog exact query is missing an exact owner path".to_owned())?;
    let structural_selector = provider_owned_structural_selector(language_id, provider_args)
        .ok_or_else(|| "catalog exact query is missing an exact structural selector".to_owned())?;
    let projection_kind = direct_projection_kind(provider_args)?;
    let canonical_workspace = project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            project_root.display()
        )
    })?;
    let Some(canonical_owner) = canonical_exact_owner(&canonical_workspace, owner_path)? else {
        return write_owner_missing_resolution(
            language_id,
            &provider.provider_id,
            provider_args,
            owner_path,
            structural_selector,
        );
    };
    let Some(source) = read_exact_owner(&canonical_owner, owner_path)? else {
        return write_owner_missing_resolution(
            language_id,
            &provider.provider_id,
            provider_args,
            owner_path,
            structural_selector,
        );
    };
    trace("catalog-owner-read", started);
    let source_content_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&source)
            .as_str()
            .to_owned();
    let request = ProviderNativeExactRequest {
        schema_id: "agent.semantic-protocols.provider-native-exact-request",
        schema_version: "1",
        language_id,
        provider_id: &provider.provider_id,
        structural_selector,
        owner_path,
        projection_kind,
        generation_identity_digest: &source_content_digest,
        parser_identity_digest: &provider.exact_parser_identity_digest,
        query_pack_digest: &provider.exact_query_pack_identity_digest,
        source_digest: &source_content_digest,
        source_byte_length: source.len(),
        source_encoding: "base64",
        source_bytes_base64: encode_base64(&source),
        transport: "stdin-json",
    };
    let request = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode catalog exact-selector request: {error}"))?;
    let native_exact = agent_semantic_hook::registered_provider_method_invocation_v1(
        language_id,
        &provider.provider_id,
        "query/exact-selector-native-v1",
    )?
    .is_some();
    let mut invocation = provider.argv_prefix.clone();
    if invocation.is_empty() {
        return Err(format!(
            "Global provider catalog exact method has empty argv: language={language_id}"
        ));
    }
    invocation.extend(direct_provider_process_args(provider_args));
    if !native_exact {
        return Err(format!(
            "catalog provider {} does not declare query/exact-selector-native-v1 required by typed projection `{projection_kind}`",
            provider.provider_id
        ));
    }
    let _program = invocation.first().ok_or_else(|| {
        format!("Global provider catalog exact method has empty argv: language={language_id}")
    })?;
    if !invocation.iter().any(|argument| argument == "--json") {
        invocation.push("--json".to_owned());
    }
    if !invocation
        .iter()
        .any(|argument| argument == "--asp-exact-request-stdin")
    {
        invocation.push("--asp-exact-request-stdin".to_owned());
    }
    if !invocation
        .windows(2)
        .any(|pair| pair[0] == "--asp-provider-id")
    {
        invocation.extend(["--asp-provider-id".to_owned(), provider.provider_id.clone()]);
    }
    if !invocation
        .windows(2)
        .any(|pair| pair[0] == "--asp-parser-identity-digest")
    {
        invocation.extend([
            "--asp-parser-identity-digest".to_owned(),
            provider.execution_command_digest.clone(),
        ]);
    }
    if !invocation
        .windows(2)
        .any(|pair| pair[0] == "--asp-query-pack-digest")
    {
        invocation.extend([
            "--asp-query-pack-digest".to_owned(),
            provider.query_pack_digest.clone(),
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
    let native: ProviderNativeExactResponse = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| format!("failed to decode catalog exact-selector output: {error}"))?;
    trace("catalog-provider-decode", started);
    match native {
        ProviderNativeExactResponse::Resolution(resolution) => {
            validate_resolution(
                &resolution,
                language_id,
                &provider.provider_id,
                owner_path,
                structural_selector,
            )?;
            trace("catalog-resolution-validate", started);
            let result =
                write_resolution_output(&resolution, provider_args, Some(output.stdout.as_ref()));
            trace("catalog-resolution-written", started);
            result
        }
        ProviderNativeExactResponse::Projection(projection) => {
            validate_projection(
                &projection,
                language_id,
                &provider.provider_id,
                provider_args,
                owner_path,
                structural_selector,
                &source_content_digest,
                &source_content_digest,
                &provider.exact_parser_identity_digest,
                &provider.exact_query_pack_identity_digest,
                &source,
            )?;
            trace("catalog-projection-validate", started);
            let result = if provider_args.iter().any(|arg| arg == "--json") {
                std::io::Write::write_all(&mut std::io::stdout().lock(), output.stdout.as_ref())
                    .map_err(|error| {
                        format!("failed to write catalog exact-selector packet: {error}")
                    })
            } else {
                write_projection_output(&projection)
            };
            trace("catalog-projection-written", started);
            result
        }
    }
}

pub(crate) fn direct_projection_kind(provider_args: &[String]) -> Result<&str, String> {
    let mut projection_kind = None;
    let mut index = 0usize;
    while index < provider_args.len() {
        let arg = provider_args[index].as_str();
        let candidate = if arg == "--projection" {
            index += 1;
            Some(
                provider_args
                    .get(index)
                    .ok_or_else(|| "exact query --projection requires a value".to_string())?
                    .as_str(),
            )
        } else {
            arg.strip_prefix("--projection=")
        };
        if let Some(candidate) = candidate {
            if !matches!(candidate, "source" | "callable-skeleton") {
                return Err(format!(
                    "unsupported exact query projection `{candidate}`; expected source or callable-skeleton"
                ));
            }
            if projection_kind.replace(candidate).is_some() {
                return Err("exact query accepts exactly one --projection".to_string());
            }
        }
        index += 1;
    }
    projection_kind.ok_or_else(|| {
        "exact query requires explicit `--projection source|callable-skeleton`".to_string()
    })
}

pub(crate) fn direct_provider_process_args(provider_args: &[String]) -> Vec<String> {
    let provider_args = provider_process_args(provider_args);
    let mut filtered = Vec::with_capacity(provider_args.len());
    let mut index = 0usize;
    while index < provider_args.len() {
        if provider_args[index] == "--projection" {
            index += 2;
            continue;
        }
        if provider_args[index].starts_with("--projection=") {
            index += 1;
            continue;
        }
        filtered.push(provider_args[index].clone());
        index += 1;
    }
    filtered
}

/// Run one canonical exact selector from one live workspace-contained owner.
pub(super) fn run_direct_exact_query(context: DirectExactQueryContext<'_>) -> Result<(), String> {
    let owner_path =
        provider_owned_structural_owner_path(context.language_id, context.provider_args)
            .ok_or_else(|| {
                "provider-owned structural query is missing an exact owner path".to_string()
            })?;
    let structural_selector =
        provider_owned_structural_selector(context.language_id, context.provider_args).ok_or_else(
            || "provider-owned structural query is missing an exact selector".to_string(),
        )?;
    let projection_kind = direct_projection_kind(context.provider_args)?;
    let canonical_workspace = context.project_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize exact-selector workspace {}: {error}",
            context.project_root.display()
        )
    })?;
    let Some(canonical_owner) = canonical_exact_owner(&canonical_workspace, owner_path)? else {
        return write_owner_missing_resolution(
            context.language_id,
            context.provider.provider_id.as_str(),
            context.provider_args,
            owner_path,
            structural_selector,
        );
    };
    let Some(source) = read_exact_owner(&canonical_owner, owner_path)? else {
        return write_owner_missing_resolution(
            context.language_id,
            context.provider.provider_id.as_str(),
            context.provider_args,
            owner_path,
            structural_selector,
        );
    };
    trace("direct-owner-read", context.started);
    let source_content_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&source)
            .as_str()
            .to_owned();
    let query_pack_bytes = serde_json::to_vec(&context.provider.query_pack_descriptor)
        .map_err(|error| format!("failed to encode activated query-pack descriptor: {error}"))?;
    let query_pack_digest =
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
            &query_pack_bytes,
        )
        .as_str()
        .to_owned();
    let request = ProviderNativeExactRequest {
        schema_id: "agent.semantic-protocols.provider-native-exact-request",
        schema_version: "1",
        language_id: context.language_id,
        provider_id: context.provider.provider_id.as_str(),
        structural_selector,
        owner_path,
        projection_kind,
        generation_identity_digest: &source_content_digest,
        parser_identity_digest: context.provider.execution_command_digest.as_str(),
        query_pack_digest: &query_pack_digest,
        source_digest: &source_content_digest,
        source_byte_length: source.len(),
        source_encoding: "base64",
        source_bytes_base64: encode_base64(&source),
        transport: "stdin-json",
    };
    let request = serde_json::to_vec(&request)
        .map_err(|error| format!("failed to encode exact-selector stdin request: {error}"))?;
    let mut provider_argv = direct_provider_process_args(context.provider_args);
    provider_argv.extend([
        "--json".to_string(),
        "--asp-provider-id".to_string(),
        context.provider.provider_id.to_string(),
        "--asp-parser-identity-digest".to_string(),
        context.provider.execution_command_digest.to_string(),
        "--asp-query-pack-digest".to_string(),
        query_pack_digest.clone(),
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
    let native: ProviderNativeExactResponse = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| {
            format!("failed to decode direct exact-selector provider output: {error}")
        })?;
    match native {
        ProviderNativeExactResponse::Resolution(resolution) => {
            validate_resolution(
                &resolution,
                context.language_id,
                context.provider.provider_id.as_str(),
                owner_path,
                structural_selector,
            )?;
            write_resolution_output(
                &resolution,
                context.provider_args,
                Some(output.stdout.as_ref()),
            )
        }
        ProviderNativeExactResponse::Projection(projection) => {
            validate_projection(
                &projection,
                context.language_id,
                context.provider.provider_id.as_str(),
                context.provider_args,
                owner_path,
                structural_selector,
                &source_content_digest,
                &source_content_digest,
                context.provider.execution_command_digest.as_str(),
                &query_pack_digest,
                &source,
            )?;
            if context.provider_args.iter().any(|arg| arg == "--json") {
                std::io::Write::write_all(&mut std::io::stdout().lock(), output.stdout.as_ref())
                    .map_err(|error| {
                        format!("failed to write direct exact-selector packet: {error}")
                    })
            } else {
                write_projection_output(&projection)
            }
        }
    }
}

fn canonical_exact_owner(
    workspace: &Path,
    owner_path: &str,
) -> Result<Option<std::path::PathBuf>, String> {
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
    let canonical_owner = match workspace.join(owner).canonicalize() {
        Ok(canonical_owner) => canonical_owner,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to resolve exact-selector owner inside workspace: ownerPath={owner_path} error={error}"
            ));
        }
    };
    if !canonical_owner.starts_with(workspace) || !canonical_owner.is_file() {
        return Err(format!(
            "exact-selector owner escaped or is not a file in the provider workspace: ownerPath={owner_path}"
        ));
    }
    Ok(Some(canonical_owner))
}

fn write_projection_output(native: &ProviderNativeExactProjection) -> Result<(), String> {
    let output = match native.projection_mode {
        ProviderNativeExactProjectionMode::Source => native
            .projection_text
            .as_deref()
            .ok_or_else(|| "source exact projection is missing projectionText".to_string())?
            .as_bytes()
            .to_vec(),
        ProviderNativeExactProjectionMode::CallableSkeleton => {
            let payload = native.projection_payload.as_ref().ok_or_else(|| {
                "callable-skeleton exact projection is missing projectionPayload".to_string()
            })?;
            let mut output = serde_json::to_vec(payload)
                .map_err(|error| format!("failed to encode callable-skeleton payload: {error}"))?;
            output.push(b'\n');
            output
        }
    };
    std::io::Write::write_all(&mut std::io::stdout().lock(), &output)
        .map_err(|error| format!("failed to write direct exact-selector projection: {error}"))
}

fn write_owner_missing_resolution(
    language_id: &str,
    provider_id: &str,
    provider_args: &[String],
    owner_path: &str,
    structural_selector: &str,
) -> Result<(), String> {
    let selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::parse(
            structural_selector,
        )?;
    let resolution = ProviderNativeExactResolution {
        schema_id: "agent.semantic-protocols.provider-native-exact-projection".to_owned(),
        schema_version: "1".to_owned(),
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        owner_path: owner_path.to_owned(),
        requested_structural_selector: structural_selector.to_owned(),
        resolution_state: "owner-missing".to_owned(),
        reason_kind: "owner-not-in-workspace".to_owned(),
        root_digest: None,
        item_kind: selector.kind.as_str().to_owned(),
        item_name: selector.symbol.as_str().to_owned(),
        candidates: Vec::new(),
        actual_kinds: Vec::new(),
        recommended_next: ProviderNativeExactRecommendedNext {
            command: format!(
                "asp {language_id} search pipe '{}' --workspace . --view seeds",
                selector.symbol.as_str()
            ),
        },
    };
    validate_resolution(
        &resolution,
        language_id,
        provider_id,
        owner_path,
        structural_selector,
    )?;
    write_resolution_output(&resolution, provider_args, None)
}

fn validate_resolution(
    resolution: &ProviderNativeExactResolution,
    language_id: &str,
    provider_id: &str,
    owner_path: &str,
    structural_selector: &str,
) -> Result<(), String> {
    validate_projection_field(
        "schemaId",
        &resolution.schema_id,
        "agent.semantic-protocols.provider-native-exact-projection",
    )?;
    validate_projection_field("schemaVersion", &resolution.schema_version, "1")?;
    validate_projection_field("languageId", &resolution.language_id, language_id)?;
    validate_projection_field("providerId", &resolution.provider_id, provider_id)?;
    validate_projection_field("ownerPath", &resolution.owner_path, owner_path)?;
    validate_projection_field(
        "requestedStructuralSelector",
        &resolution.requested_structural_selector,
        structural_selector,
    )?;
    if !matches!(
        resolution.resolution_state.as_str(),
        "item-missing" | "owner-missing" | "kind-mismatch" | "ambiguous"
    ) {
        return Err(format!(
            "exact-selector semantic resolutionState is invalid: {}",
            resolution.resolution_state
        ));
    }
    if resolution.reason_kind.is_empty() {
        return Err("exact-selector semantic reasonKind is empty".to_owned());
    }
    let selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::parse(
            structural_selector,
        )?;
    validate_projection_field("itemKind", &resolution.item_kind, selector.kind.as_str())?;
    validate_projection_field("itemName", &resolution.item_name, selector.symbol.as_str())?;
    if resolution.recommended_next.command.is_empty() {
        return Err("exact-selector semantic recommendedNext.command is empty".to_owned());
    }
    for candidate in &resolution.candidates {
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::parse(
            candidate,
        )
        .map_err(|error| {
            format!("exact-selector semantic candidate is not canonical: {candidate}: {error}")
        })?;
    }
    Ok(())
}

fn write_resolution_output(
    resolution: &ProviderNativeExactResolution,
    provider_args: &[String],
    raw_json: Option<&[u8]>,
) -> Result<(), String> {
    if provider_args.iter().any(|arg| arg == "--json") {
        if let Some(raw_json) = raw_json {
            return std::io::Write::write_all(&mut std::io::stdout().lock(), raw_json).map_err(
                |error| format!("failed to write exact-selector semantic packet: {error}"),
            );
        }
        let mut packet = serde_json::to_vec(resolution)
            .map_err(|error| format!("failed to encode exact-selector semantic packet: {error}"))?;
        packet.push(b'\n');
        return std::io::Write::write_all(&mut std::io::stdout().lock(), &packet)
            .map_err(|error| format!("failed to write exact-selector semantic packet: {error}"));
    }
    let output = format!(
        "exact source query state={} reasonKind={} ownerPath={} itemKind={} itemName={} candidates={} actualKinds={} next={}\n",
        resolution.resolution_state,
        resolution.reason_kind,
        resolution.owner_path,
        resolution.item_kind,
        resolution.item_name,
        resolution.candidates.join(","),
        resolution.actual_kinds.join(","),
        resolution.recommended_next.command,
    );
    std::io::Write::write_all(&mut std::io::stdout().lock(), output.as_bytes())
        .map_err(|error| format!("failed to write exact-selector semantic result: {error}"))
}

fn validate_projection(
    native: &ProviderNativeExactProjection,
    language_id: &str,
    provider_id: &str,
    provider_args: &[String],
    owner_path: &str,
    structural_selector: &str,
    source_content_digest: &str,
    generation_identity_digest: &str,
    parser_identity_digest: &str,
    query_pack_digest: &str,
    source: &[u8],
) -> Result<(), String> {
    let projection_mode = match direct_projection_kind(provider_args)? {
        "source" => ProviderNativeExactProjectionMode::Source,
        "callable-skeleton" => ProviderNativeExactProjectionMode::CallableSkeleton,
        _ => unreachable!("direct_projection_kind validates its result"),
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
    match projection_mode {
        ProviderNativeExactProjectionMode::Source => {
            if native.projection_payload.is_some() {
                return Err(
                    "source exact-selector projection unexpectedly carries projectionPayload"
                        .to_string(),
                );
            }
            let projection_text = native.projection_text.as_deref().ok_or_else(|| {
                "source exact-selector projection is missing projectionText".to_string()
            })?;
            let start = usize::try_from(native.source_byte_start)
                .map_err(|_| "direct exact-selector byte start overflow".to_string())?;
            let end = usize::try_from(native.source_byte_end)
                .map_err(|_| "direct exact-selector byte end overflow".to_string())?;
            if projection_text.as_bytes() != &source[start..end] {
                return Err(
                    "direct exact-selector projection is not the parser-owned source byte slice"
                        .to_string(),
                );
            }
        }
        ProviderNativeExactProjectionMode::CallableSkeleton => {
            if native.projection_text.is_some() {
                return Err(
                    "callable-skeleton exact projection unexpectedly carries projectionText"
                        .to_string(),
                );
            }
            let payload = native.projection_payload.as_ref().ok_or_else(|| {
                "callable-skeleton exact projection is missing projectionPayload".to_string()
            })?;
            payload
                .validate()
                .map_err(|error| format!("invalid callable-skeleton projection: {error}"))?;
            validate_projection_field("payload.languageId", &payload.language_id, language_id)?;
            validate_projection_field("payload.providerId", &payload.provider_id, provider_id)?;
            validate_projection_field(
                "payload.rootSelector.selector",
                &payload.root_selector.selector,
                structural_selector,
            )?;
            validate_projection_field(
                "payload.rootSelector.generationIdentityDigest",
                &payload.root_selector.generation_identity_digest,
                generation_identity_digest,
            )?;
            validate_projection_field(
                "payload.rootSelector.parserIdentityDigest",
                &payload.root_selector.parser_identity_digest,
                parser_identity_digest,
            )?;
            validate_projection_field(
                "payload.rootSelector.queryPackDigest",
                &payload.root_selector.query_pack_digest,
                query_pack_digest,
            )?;
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

fn read_exact_owner(canonical_owner: &Path, owner_path: &str) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(canonical_owner) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "failed to read exact-selector owner bytes: ownerPath={owner_path} error={error}"
        )),
    }
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
