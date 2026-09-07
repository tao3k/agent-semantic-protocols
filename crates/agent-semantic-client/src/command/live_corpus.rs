// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Qualify and atomically publish provider live-corpus artifacts.

use agent_semantic_provider_protocol::ProviderRegisterOperation;
use agent_semantic_provider_protocol::ProviderRegisterRequest;
use agent_semantic_provider_protocol::ProviderRegisterResult;
use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use agent_semantic_runtime::LiveCorpusArtifactIdentity;
use agent_semantic_runtime::LiveCorpusGitCheckoutQualification;
use agent_semantic_runtime::LiveCorpusLanguageExtensionEvidence;
use agent_semantic_runtime::live_corpus_artifact_manifest;
use agent_semantic_runtime::live_corpus_artifact_paths;
use agent_semantic_runtime::live_corpus_git_checkout_is_clean;
use agent_semantic_runtime::live_corpus_git_repository_paths;
use agent_semantic_runtime::live_corpus_lock_digest;
use agent_semantic_runtime::qualify_live_corpus_git_checkout;
use agent_semantic_runtime::qualify_live_corpus_language_extensions;
use agent_semantic_runtime::resolve_state_home;
use agent_semantic_runtime::sync_live_corpus_git_checkout;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const DEFAULT_LOCK_PATH: &str = "benchmarks/large-library-runtime-corpora.json";

#[derive(Debug, Eq, PartialEq)]
struct LiveCorpusMaterializedSourceIdentity {
    head_revision: String,
    git_tree: String,
    source_merkle_root: String,
}

fn materialized_source_identity(
    checkout: LiveCorpusGitCheckoutQualification,
    runtime_source_root_digest: String,
) -> Result<LiveCorpusMaterializedSourceIdentity, String> {
    if runtime_source_root_digest.trim().is_empty() {
        return Err("ASP Server returned an empty canonical source root digest".to_owned());
    }
    Ok(LiveCorpusMaterializedSourceIdentity {
        head_revision: checkout.head_revision,
        git_tree: checkout.git_tree,
        source_merkle_root: runtime_source_root_digest,
    })
}
#[path = "live_corpus_qualification/mod.rs"]
mod qualification;
const BUILDER_ID: &str = "asp-live-corpus";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusLock {
    schema_id: String,
    schema_version: String,
    corpora: Vec<LiveCorpusLockEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusLockEntry {
    resource_id: String,
    scenario_id: String,
    provider_id: String,
    language: String,
    repository: String,
    git: LiveCorpusGitLock,
    directory: String,
    environment: String,
    #[serde(default)]
    admission: Option<LiveCorpusExtensionAdmission>,
    inputs: LiveCorpusInputs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusGitLock {
    remote: String,
    revision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusExtensionAdmission {
    extension_authority: String,
    minimum_matching_files: usize,
    minimum_matching_file_ratio: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusInputs {
    owner: String,
    query: String,
    dependency: String,
}

#[derive(Debug)]
struct MaterializeRequest {
    resource_id: String,
    source: PathBuf,
    lock_path: PathBuf,
    json: bool,
}

#[derive(Debug)]
struct PathRequest {
    resource_id: String,
    lock_path: PathBuf,
    json: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusQualification {
    schema_id: String,
    schema_version: String,
    artifact_digest: String,
    materialization_authority: String,
    source_path: String,
    head_revision: String,
    git_tree: String,
    source_merkle_root: String,
    language_extension_evidence: LiveCorpusLanguageExtensionEvidence,
    clean: bool,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusMaterializeReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    resource_id: String,
    provider_id: String,
    language_id: String,
    artifact_digest: String,
    artifact_path: String,
    current_pointer: String,
    matching_file_count: usize,
    candidate_language_file_count: usize,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusPathReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    resource_id: String,
    remote: String,
    revision: String,
    repository_path: String,
    checkout_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusSyncReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    resource_id: String,
    remote: String,
    revision: String,
    checkout_path: String,
    status: &'static str,
}

pub(crate) async fn run_live_corpus_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("materialize") => materialize(parse_materialize_request(&args[1..])?).await,
        Some("qualify") => {
            // Reject malformed CLI input before touching Runtime authority. This
            // keeps argument validation deterministic even when no Runtime
            // endpoint is available.
            qualification::validate_args(&args[1..])?;
            let mut ready =
                crate::server::runtime_server::ensure_healthy_runtime_server_for_bounded_operation(
                )
                .await?;
            let transaction = ready.resident_transaction.take().ok_or_else(|| {
                "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime bootstrap returned Healthy without its resident transaction"
                    .to_owned()
            })?;
            qualification::run(
                &args[1..],
                crate::AspClientRuntimeHandoff::try_from(&transaction)?,
            )
            .await
        }
        Some("path") => print_path(parse_path_request(&args[1..])?),
        Some("sync") => sync_resource(parse_resource_request(&args[1..], sync_usage)?),
        Some("help" | "--help" | "-h") => {
            println!("{}", root_usage());
            Ok(())
        }
        _ => Err(root_usage()),
    }
}

fn parse_path_request(args: &[String]) -> Result<PathRequest, String> {
    parse_resource_request(args, path_usage)
}

fn parse_resource_request(
    args: &[String],
    command_usage: fn() -> String,
) -> Result<PathRequest, String> {
    let mut resource_id = None;
    let mut lock_path = PathBuf::from(DEFAULT_LOCK_PATH);
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--resource" => {
                index += 1;
                resource_id = args.get(index).cloned();
            }
            "--lock" => {
                index += 1;
                lock_path = args
                    .get(index)
                    .map(PathBuf::from)
                    .ok_or_else(command_usage)?;
            }
            "--json" => json = true,
            argument => {
                return Err(format!(
                    "unknown live-corpus option `{argument}`\n{}",
                    command_usage()
                ));
            }
        }
        index += 1;
    }
    Ok(PathRequest {
        resource_id: resource_id.ok_or_else(command_usage)?,
        lock_path,
        json,
    })
}

fn parse_materialize_request(args: &[String]) -> Result<MaterializeRequest, String> {
    let mut resource_id = None;
    let mut source = None;
    let mut lock_path = PathBuf::from(DEFAULT_LOCK_PATH);
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--resource" => {
                index += 1;
                resource_id = args.get(index).cloned();
            }
            "--source" => {
                index += 1;
                source = args.get(index).map(PathBuf::from);
            }
            "--lock" => {
                index += 1;
                lock_path = args
                    .get(index)
                    .map(PathBuf::from)
                    .ok_or_else(materialize_usage)?;
            }
            "--json" => json = true,
            argument => {
                return Err(format!(
                    "unknown live-corpus option `{argument}`\n{}",
                    materialize_usage()
                ));
            }
        }
        index += 1;
    }
    Ok(MaterializeRequest {
        resource_id: resource_id.ok_or_else(materialize_usage)?,
        source: source.ok_or_else(materialize_usage)?,
        lock_path,
        json,
    })
}

async fn materialize(request: MaterializeRequest) -> Result<(), String> {
    let total_started = std::time::Instant::now();
    let mut step_started = total_started;
    let lock = load_lock(&request.lock_path)?;
    let corpus = unique_resource(&lock.corpora, &request.resource_id)?;
    emit_live_corpus_timing("lock", &mut step_started);
    let source = request
        .source
        .canonicalize()
        .map_err(|error| format!("failed to resolve live-corpus source: {error}"))?;
    let state_home = resolve_state_home()?;
    let repository = live_corpus_git_repository_paths(&state_home, &corpus.git.remote)?;
    let checkout =
        qualify_live_corpus_git_checkout(&source, &corpus.git.remote, &corpus.git.revision)?;
    let expected_source_path = repository
        .repository_dir
        .join("checkouts")
        .join(&corpus.git.revision);
    let expected_source = expected_source_path
        .canonicalize()
        .unwrap_or_else(|_| expected_source_path.clone());
    emit_live_corpus_timing("git-identity", &mut step_started);
    if source != expected_source {
        return Err(format!(
            "live corpus source is outside the canonical State Home checkout: expected={} actual={}",
            expected_source_path.display(),
            source.display()
        ));
    }
    if !live_corpus_git_checkout_is_clean(&source)? {
        return Err("live corpus checkout is dirty; qualification is fail-closed".to_string());
    }
    emit_live_corpus_timing("git-status", &mut step_started);

    let mut ready =
        crate::server::runtime_server::ensure_healthy_runtime_server_for_bounded_operation()
            .await?;
    let transaction = ready.resident_transaction.take().ok_or_else(|| {
        "reasonKind=runtime-client-handoff-unavailable failureLayer=runtime-resident-transaction Runtime bootstrap returned Healthy without its resident transaction"
            .to_owned()
    })?;
    let handoff = crate::AspClientRuntimeHandoff::try_from(&transaction)?;
    let registration =
        runtime_provider_registration(handoff.provider_socket_addr(), corpus).await?;
    if registration.provider_id != corpus.provider_id {
        return Err(format!(
            "live corpus provider mismatch: lock={} registration={}",
            corpus.provider_id, registration.provider_id
        ));
    }
    let source_inventory = registration.source_inventory()?;
    let registry_extensions = source_inventory.source_extensions.clone();
    let extension_evidence = qualify_live_corpus_language_extensions(
        &source,
        &source_inventory.source_extensions,
        &registry_extensions,
    )?;
    validate_extension_admission(corpus, &extension_evidence)?;
    emit_live_corpus_timing("provider-registration", &mut step_started);
    emit_live_corpus_timing("extension-evidence", &mut step_started);

    let client = agent_semantic_client::RuntimeLanguageCommandClient;
    let response = agent_semantic_client::LanguageCommandClient::dispatch(
        &client,
        agent_semantic_client::LanguageCommandRequest {
            language_id: agent_semantic_client::LanguageId::new(corpus.language.as_str()),
            operation: agent_semantic_client::LanguageCommandOperation::Search(
                agent_semantic_client_protocol::AspClientSearchRequest::playbook(
                    "conceptual",
                    corpus.inputs.query.clone(),
                ),
            ),
            project_root: source.clone(),
            machine_readable: true,
        },
    )
    .await?;
    let generation = serde_json::from_value::<agent_semantic_search::SearchPlaybookReceipt>(
        response.require_ready_payload()?,
    )
    .map_err(|error| format!("decode Live Corpus public search payload: {error}"))?;
    generation.validate()?;
    let materialized_source =
        materialized_source_identity(checkout, generation.source_root_digest)?;
    emit_live_corpus_timing("runtime-generation", &mut step_started);

    let resource_lock =
        serde_json::to_vec(corpus).map_err(|error| format!("failed to encode lock: {error}"))?;
    let lock_digest = live_corpus_lock_digest(&resource_lock);
    let identity = LiveCorpusArtifactIdentity {
        lock_digest: &lock_digest,
        resource_id: &corpus.resource_id,
        provider_id: &corpus.provider_id,
        language_id: &corpus.language,
        builder_id: BUILDER_ID,
        revision: &materialized_source.head_revision,
        git_tree: &materialized_source.git_tree,
        source_merkle_root: &materialized_source.source_merkle_root,
    };
    let paths = live_corpus_artifact_paths(&state_home, &repository, &identity)?;
    let manifest =
        live_corpus_artifact_manifest(&corpus.git.remote, &repository, &identity, &paths)?;
    let qualification = LiveCorpusQualification {
        schema_id: "agent.semantic-protocols.live-corpus-artifact-qualification".to_owned(),
        schema_version: "1".to_owned(),
        artifact_digest: paths.artifact_digest.clone(),
        materialization_authority: "developer-gix".to_owned(),
        source_path: source.display().to_string(),
        head_revision: materialized_source.head_revision,
        git_tree: materialized_source.git_tree,
        source_merkle_root: materialized_source.source_merkle_root,
        language_extension_evidence: extension_evidence.clone(),
        clean: true,
        status: "qualified".to_owned(),
    };
    emit_live_corpus_timing("artifact-identity", &mut step_started);
    publish_immutable_json(&paths.manifest_path, &manifest)?;
    publish_immutable_json(&paths.qualification_path, &qualification)?;
    publish_current_pointer(&paths.current_pointer, &paths.artifact_dir)?;
    emit_live_corpus_timing("publication", &mut step_started);

    let receipt = LiveCorpusMaterializeReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-materialize-receipt",
        schema_version: "1",
        resource_id: corpus.resource_id.clone(),
        provider_id: corpus.provider_id.clone(),
        language_id: corpus.language.clone(),
        artifact_digest: paths.artifact_digest,
        artifact_path: paths.artifact_dir.display().to_string(),
        current_pointer: paths.current_pointer.display().to_string(),
        matching_file_count: extension_evidence.matching_file_count,
        candidate_language_file_count: extension_evidence.candidate_language_file_count,
        status: "qualified",
    };
    if request.json {
        println!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("failed to encode materialize receipt: {error}"))?
        );
    } else {
        println!(
            "[live-corpus] resource={} language={} provider={} matchingFiles={} candidateLanguageFiles={} artifact={} status=qualified",
            receipt.resource_id,
            receipt.language_id,
            receipt.provider_id,
            receipt.matching_file_count,
            receipt.candidate_language_file_count,
            receipt.artifact_digest
        );
    }
    if env::var_os("ASP_LIVE_CORPUS_TIMINGS").is_some() {
        eprintln!(
            "[live-corpus-timing] step=total stepMs={:.3}",
            total_started.elapsed().as_secs_f64() * 1_000.0
        );
    }
    Ok(())
}

async fn runtime_provider_registration(
    provider_endpoint: std::net::SocketAddr,
    corpus: &LiveCorpusLockEntry,
) -> Result<ProviderRegistrationDocument, String> {
    let request = ProviderRegisterRequest {
        schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
            .to_owned(),
        expected_generation: None,
        request: ProviderRegisterOperation::List,
    };
    let response =
        agent_semantic_provider_transport::grpc_session::call_runtime_provider_register_tcp(
            provider_endpoint,
            &request,
        )
        .await?;
    response.validate()?;
    let snapshot = match response.result {
        ProviderRegisterResult::Snapshot { snapshot } => snapshot,
        ProviderRegisterResult::GenerationConflict { actual_generation } => {
            return Err(format!(
                "Runtime provider register list returned generation conflict: actualGeneration={actual_generation}"
            ));
        }
        ProviderRegisterResult::Rejected {
            reason_kind,
            message,
        } => {
            return Err(format!(
                "Runtime provider register list rejected: reasonKind={reason_kind} message={message}"
            ));
        }
    };
    snapshot.validate()?;
    let generation = snapshot.generation;
    let digest = snapshot.digest.clone();
    snapshot
        .providers
        .into_iter()
        .find(|provider| {
            provider.language_id == corpus.language && provider.provider_id == corpus.provider_id
        })
        .ok_or_else(|| {
            format!(
                "Runtime provider register is missing live-corpus provider: language={} provider={} generation={} digest={}",
                corpus.language, corpus.provider_id, generation, digest
            )
        })
}

fn emit_live_corpus_timing(step: &str, started: &mut std::time::Instant) {
    if env::var_os("ASP_LIVE_CORPUS_TIMINGS").is_some() {
        eprintln!(
            "[live-corpus-timing] step={step} stepMs={:.3}",
            started.elapsed().as_secs_f64() * 1_000.0
        );
    }
    *started = std::time::Instant::now();
}

fn print_path(request: PathRequest) -> Result<(), String> {
    let lock = load_lock(&request.lock_path)?;
    let corpus = unique_resource(&lock.corpora, &request.resource_id)?;
    let state_home = resolve_state_home()?;
    let repository = live_corpus_git_repository_paths(&state_home, &corpus.git.remote)?;
    let checkout = repository
        .repository_dir
        .join("checkouts")
        .join(&corpus.git.revision);
    let receipt = LiveCorpusPathReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-path-receipt",
        schema_version: "1",
        resource_id: corpus.resource_id.clone(),
        remote: corpus.git.remote.clone(),
        revision: corpus.git.revision.clone(),
        repository_path: repository.repository_dir.display().to_string(),
        checkout_path: checkout.display().to_string(),
    };
    if request.json {
        println!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("failed to encode path receipt: {error}"))?
        );
    } else {
        println!(
            "[live-corpus-path] resource={} remote={} revision={} checkout={}",
            receipt.resource_id, receipt.remote, receipt.revision, receipt.checkout_path
        );
    }
    Ok(())
}

fn sync_resource(request: PathRequest) -> Result<(), String> {
    let lock = load_lock(&request.lock_path)?;
    let corpus = unique_resource(&lock.corpora, &request.resource_id)?;
    let state_home = resolve_state_home()?;
    let checkout =
        sync_live_corpus_git_checkout(&state_home, &corpus.git.remote, &corpus.git.revision)?;
    let receipt = LiveCorpusSyncReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-sync-receipt",
        schema_version: "1",
        resource_id: corpus.resource_id.clone(),
        remote: corpus.git.remote.clone(),
        revision: corpus.git.revision.clone(),
        checkout_path: checkout.checkout_dir.display().to_string(),
        status: if checkout.reused {
            "reused"
        } else {
            "materialized"
        },
    };
    if request.json {
        println!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("failed to encode sync receipt: {error}"))?
        );
    } else {
        println!(
            "[live-corpus-sync] resource={} revision={} checkout={} status={}",
            receipt.resource_id, receipt.revision, receipt.checkout_path, receipt.status
        );
    }
    Ok(())
}

fn load_lock(path: &Path) -> Result<LiveCorpusLock, String> {
    let lock_bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read live-corpus lock {}: {error}",
            path.display()
        )
    })?;
    let lock: LiveCorpusLock = serde_json::from_slice(&lock_bytes)
        .map_err(|error| format!("failed to decode live-corpus lock: {error}"))?;
    if lock.schema_id != "agent.semantic-protocols.semantic-sandtable-large-library-corpora"
        || lock.schema_version != "1"
    {
        return Err("live-corpus lock contract mismatch".to_string());
    }
    Ok(lock)
}

fn unique_resource<'a>(
    corpora: &'a [LiveCorpusLockEntry],
    resource_id: &str,
) -> Result<&'a LiveCorpusLockEntry, String> {
    let matches = corpora
        .iter()
        .filter(|corpus| corpus.resource_id == resource_id)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [corpus] => Ok(*corpus),
        [] => Err(format!("live corpus resource is not locked: {resource_id}")),
        _ => Err(format!("live corpus resource is duplicated: {resource_id}")),
    }
}

fn validate_extension_admission(
    corpus: &LiveCorpusLockEntry,
    evidence: &LiveCorpusLanguageExtensionEvidence,
) -> Result<(), String> {
    let Some(admission) = corpus.admission.as_ref() else {
        return Ok(());
    };
    if admission.extension_authority != evidence.authority {
        return Err("live corpus extension authority mismatch".to_string());
    }
    let ratio = evidence.matching_file_count as f64 / evidence.candidate_language_file_count as f64;
    if evidence.matching_file_count < admission.minimum_matching_files
        || ratio < admission.minimum_matching_file_ratio
    {
        return Err(format!(
            "live corpus language-extension admission failed: matchingFiles={} requiredFiles={} ratio={ratio:.6} requiredRatio={:.6}",
            evidence.matching_file_count,
            admission.minimum_matching_files,
            admission.minimum_matching_file_ratio
        ));
    }
    Ok(())
}

fn publish_immutable_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to encode live-corpus artifact: {error}"))?;
    bytes.push(b'\n');
    if path.is_file() {
        let existing = fs::read(path)
            .map_err(|error| format!("failed to read immutable artifact: {error}"))?;
        return if existing == bytes {
            Ok(())
        } else {
            Err(format!(
                "immutable live-corpus artifact collision: {}",
                path.display()
            ))
        };
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("artifact path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create artifact directory: {error}"))?;
    let temporary = unique_temporary_path(parent, "publish");
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("failed to create artifact temporary file: {error}"))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("failed to persist artifact temporary file: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to publish immutable artifact: {error}"))
}

#[cfg(unix)]
fn publish_current_pointer(current: &Path, artifact: &Path) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let parent = current
        .parent()
        .ok_or_else(|| format!("current pointer has no parent: {}", current.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create current-pointer directory: {error}"))?;
    let temporary = unique_temporary_path(parent, "current");
    symlink(artifact, &temporary)
        .map_err(|error| format!("failed to create current-pointer symlink: {error}"))?;
    fs::rename(&temporary, current)
        .map_err(|error| format!("failed to publish current pointer atomically: {error}"))
}

#[cfg(not(unix))]
fn publish_current_pointer(_current: &Path, _artifact: &Path) -> Result<(), String> {
    Err("live-corpus current-pointer publication currently requires Unix symlinks".to_string())
}

fn unique_temporary_path(parent: &Path, prefix: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    parent.join(format!(".{prefix}-{}-{nonce}", std::process::id()))
}

fn root_usage() -> String {
    super::cli_help::live_corpus_command()
        .render_long_help()
        .to_string()
}

fn path_usage() -> String {
    format!(
        "usage: asp live-corpus path --resource <resource-id> [--lock <path>] [--json]\ndefaultLock={DEFAULT_LOCK_PATH}"
    )
}

fn sync_usage() -> String {
    format!(
        "usage: asp live-corpus sync --resource <resource-id> [--lock <path>] [--json]\ndefaultLock={DEFAULT_LOCK_PATH}"
    )
}

fn materialize_usage() -> String {
    format!(
        "usage: asp live-corpus materialize --resource <resource-id> --source <canonical-state-home-checkout> [--lock <path>] [--json]\ndefaultLock={DEFAULT_LOCK_PATH}"
    )
}

#[cfg(test)]
#[path = "../../tests/unit/command/live_corpus.rs"]
mod tests;
