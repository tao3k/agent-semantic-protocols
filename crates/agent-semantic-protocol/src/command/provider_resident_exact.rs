//! One-owner, in-memory provider execution for canonical exact selectors.

use std::path::Path;
use std::time::Instant;

use super::provider_selector::{
    provider_owned_structural_owner_path, provider_owned_structural_selector,
};

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
        trace("resident-turso-selector-miss-provider-fallback", started);
        return Ok(false);
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
