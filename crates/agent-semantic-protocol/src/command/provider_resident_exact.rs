//! One-owner, in-memory provider execution for canonical exact selectors.

use std::path::Path;
use std::time::Instant;


use super::provider_selector::{
    provider_owned_structural_owner_path, provider_owned_structural_selector,
};

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

fn resident_canonical_item_selector(
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

fn write_resident_resolution(
    language_id: &str,
    provider_id: &str,
    provider_args: &[String],
    owner_path: &str,
    structural_selector: &str,
    root_digest: &str,
    resolution_state: &str,
    reason_kind: &str,
    candidates: Vec<String>,
    actual_kinds: Vec<String>,
) -> Result<(), String> {
    let selector =
        agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector::parse(
            structural_selector,
        )?;
    let next_command = if resolution_state == "owner-missing" {
        format!(
            "asp {language_id} search pipe '{}' --workspace . --view seeds",
            selector.symbol.as_str()
        )
    } else {
        format!(
            "asp {language_id} search owner '{}' items --query '{}' --workspace . --view seeds",
            owner_path,
            selector.symbol.as_str()
        )
    };
    let resolution = ProviderNativeExactResolution {
        schema_id: "agent.semantic-protocols.provider-native-exact-projection".to_owned(),
        schema_version: "1".to_owned(),
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        owner_path: owner_path.to_owned(),
        requested_structural_selector: structural_selector.to_owned(),
        resolution_state: resolution_state.to_owned(),
        reason_kind: reason_kind.to_owned(),
        root_digest: Some(root_digest.to_owned()),
        item_kind: selector.kind.as_str().to_owned(),
        item_name: selector.symbol.as_str().to_owned(),
        candidates,
        actual_kinds,
        recommended_next: ProviderNativeExactRecommendedNext {
            command: next_command,
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
    let canonical_project_root =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .root;
    let Some(canonical_item_selector) =
        resident_canonical_item_selector(structural_selector)?
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
            owner_path: owner_path.to_owned(),
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
        let (resolution_state, reason_kind) = if read.candidates.len() > 1 {
            ("ambiguous", "multiple-snapshot-items")
        } else if !read.actual_kinds.is_empty() {
            ("kind-mismatch", "owner-item-kind-mismatch")
        } else if read.requested_owner_exists {
            ("item-missing", "item-not-in-live-owner")
        } else {
            ("owner-missing", "owner-not-in-workspace")
        };
        write_resident_resolution(
            language_id,
            &provider.provider_id,
            provider_args,
            owner_path,
            structural_selector,
            &read.source_snapshot.root_digest,
            resolution_state,
            reason_kind,
            read.candidates
                .iter()
                .map(|candidate| {
                    candidate
                        .canonical_item_selector
                        .structural_selector
                        .clone()
                })
                .collect(),
            read.actual_kinds,
        )?;
        trace("resident-turso-selector-resolution", started);
        return Ok(true);
    };
    let Some(projection) = candidate.projection.as_ref() else {
        trace("resident-turso-projection-missing", started);
        return Ok(false);
    };
    if projection.structural_selector
        != candidate
            .canonical_item_selector
            .structural_selector
            .as_str()
    {
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
    trace(
        if candidate.owner_path == owner_path {
            "resident-turso-hit"
        } else {
            "resident-turso-relocated"
        },
        started,
    );
    Ok(true)
}


#[derive(Debug, clap::Parser)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
struct DirectExactQueryCli {
    #[arg(value_parser = ["query"])]
    _command: Option<String>,
    #[arg(long)]
    _selector: Option<String>,
    #[arg(long)]
    _workspace: Option<std::path::PathBuf>,
    #[arg(long, value_parser = ["source", "callable-skeleton"])]
    projection: String,
}

pub(crate) fn direct_projection_kind(provider_args: &[String]) -> Result<&'static str, String> {
    if provider_args
        .iter()
        .any(|argument| matches!(argument.as_str(), "--code" | "--names-only"))
    {
        return Err(
            "exact query requires `--projection source|callable-skeleton`; legacy --code and --names-only are unsupported"
                .to_owned(),
        );
    }
    <DirectExactQueryCli as clap::Parser>::try_parse_from(provider_args)
        .map(|query| match query.projection.as_str() {
            "source" => "source",
            "callable-skeleton" => "callable-skeleton",
            _ => unreachable!("clap validates exact query projection values"),
        })
        .map_err(|error| error.to_string())
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
        "item-missing" | "owner-missing" | "kind-mismatch" | "identity-incomplete" | "ambiguous"
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


fn trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_EXACT_QUERY_TRACE").is_some() {
        eprintln!(
            "[exact-query-trace] stage={stage} elapsedMicros={}",
            started.elapsed().as_micros()
        );
    }
}
