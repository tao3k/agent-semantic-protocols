//! Qualify and atomically publish provider live-corpus artifacts.

use agent_semantic_runtime::{
    LiveCorpusArtifactIdentity, LiveCorpusLanguageExtensionEvidenceV1,
    live_corpus_artifact_manifest, live_corpus_artifact_paths, live_corpus_git_checkout_is_clean,
    live_corpus_git_repository_paths, live_corpus_lock_digest, qualify_live_corpus_git_checkout,
    qualify_live_corpus_language_extensions, resolve_state_home, sync_live_corpus_git_checkout,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use super::provider_activation::{load_activation_for_language, provider_activation_path};

const DEFAULT_LOCK_PATH: &str = "benchmarks/large-library-runtime-corpora.v1.json";
const BUILDER_ID: &str = "asp-live-corpus";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusLockV1 {
    schema_id: String,
    schema_version: String,
    corpora: Vec<LiveCorpusLockEntryV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusLockEntryV1 {
    resource_id: String,
    scenario_id: String,
    provider_id: String,
    language: String,
    repository: String,
    git: LiveCorpusGitLockV1,
    directory: String,
    environment: String,
    #[serde(default)]
    admission: Option<LiveCorpusExtensionAdmissionV1>,
    inputs: LiveCorpusInputsV1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusGitLockV1 {
    remote: String,
    revision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusExtensionAdmissionV1 {
    extension_authority: String,
    minimum_matching_files: usize,
    minimum_matching_file_ratio: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiveCorpusInputsV1 {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusQualificationV1 {
    schema_id: &'static str,
    schema_version: &'static str,
    artifact_digest: String,
    materialization_authority: &'static str,
    source_path: String,
    head_revision: String,
    git_tree: String,
    source_merkle_root: String,
    language_extension_evidence: LiveCorpusLanguageExtensionEvidenceV1,
    clean: bool,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveCorpusMaterializeReceiptV1 {
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
struct LiveCorpusPathReceiptV1 {
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
struct LiveCorpusSyncReceiptV1 {
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

    let invocation_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let activation_path = provider_activation_path(&invocation_root);
    let runtime =
        load_activation_for_language(&activation_path, &invocation_root, &corpus.language).await?;
    emit_live_corpus_timing("activation", &mut step_started);
    let provider = runtime
        .providers
        .iter()
        .find(|provider| provider.language_id == corpus.language.as_str())
        .ok_or_else(|| format!("no activated provider for language {}", corpus.language))?;
    if provider.provider_id.as_str() != corpus.provider_id {
        return Err(format!(
            "live corpus provider mismatch: lock={} activation={}",
            corpus.provider_id, provider.provider_id
        ));
    }
    let registry_extensions = runtime
        .providers
        .iter()
        .flat_map(|provider| provider.source_extensions.iter().cloned())
        .collect::<Vec<_>>();
    let extension_evidence = qualify_live_corpus_language_extensions(
        &source,
        &provider.source_extensions,
        &registry_extensions,
    )?;
    validate_extension_admission(corpus, &extension_evidence)?;
    emit_live_corpus_timing("extension-evidence", &mut step_started);

    let resource_lock =
        serde_json::to_vec(corpus).map_err(|error| format!("failed to encode lock: {error}"))?;
    let lock_digest = live_corpus_lock_digest(&resource_lock);
    let identity = LiveCorpusArtifactIdentity {
        lock_digest: &lock_digest,
        resource_id: &corpus.resource_id,
        provider_id: &corpus.provider_id,
        language_id: &corpus.language,
        builder_id: BUILDER_ID,
        revision: &checkout.head_revision,
        git_tree: &checkout.git_tree,
        source_merkle_root: &checkout.source_merkle_root,
    };
    let paths = live_corpus_artifact_paths(&state_home, &repository, &identity)?;
    let manifest =
        live_corpus_artifact_manifest(&corpus.git.remote, &repository, &identity, &paths)?;
    let qualification = LiveCorpusQualificationV1 {
        schema_id: "agent.semantic-protocols.live-corpus-artifact-qualification",
        schema_version: "1",
        artifact_digest: paths.artifact_digest.clone(),
        materialization_authority: "developer-gix",
        source_path: source.display().to_string(),
        head_revision: checkout.head_revision,
        git_tree: checkout.git_tree,
        source_merkle_root: checkout.source_merkle_root,
        language_extension_evidence: extension_evidence.clone(),
        clean: true,
        status: "qualified",
    };
    emit_live_corpus_timing("artifact-identity", &mut step_started);
    publish_immutable_json(&paths.manifest_path, &manifest)?;
    publish_immutable_json(&paths.qualification_path, &qualification)?;
    publish_current_pointer(&paths.current_pointer, &paths.artifact_dir)?;
    emit_live_corpus_timing("publication", &mut step_started);

    let receipt = LiveCorpusMaterializeReceiptV1 {
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
    let receipt = LiveCorpusPathReceiptV1 {
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
    let receipt = LiveCorpusSyncReceiptV1 {
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

fn load_lock(path: &Path) -> Result<LiveCorpusLockV1, String> {
    let lock_bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read live-corpus lock {}: {error}",
            path.display()
        )
    })?;
    let lock: LiveCorpusLockV1 = serde_json::from_slice(&lock_bytes)
        .map_err(|error| format!("failed to decode live-corpus v1 lock: {error}"))?;
    if lock.schema_id != "agent.semantic-protocols.semantic-sandtable-large-library-corpora"
        || lock.schema_version != "1"
    {
        return Err("live-corpus lock contract mismatch".to_string());
    }
    Ok(lock)
}

fn unique_resource<'a>(
    corpora: &'a [LiveCorpusLockEntryV1],
    resource_id: &str,
) -> Result<&'a LiveCorpusLockEntryV1, String> {
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
    corpus: &LiveCorpusLockEntryV1,
    evidence: &LiveCorpusLanguageExtensionEvidenceV1,
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
    format!(
        "usage: asp live-corpus <path|sync> --resource <resource-id> [--lock <path>] [--json]\n       asp live-corpus materialize --resource <resource-id> --source <canonical-state-home-checkout> [--lock <path>] [--json]\ndefaultLock={DEFAULT_LOCK_PATH}"
    )
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
