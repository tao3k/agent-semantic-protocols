use std::path::{Path, PathBuf};

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceRuntimeSelectorRead,
};
use serde::{Deserialize, Serialize};

const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-search-query-qualification.v1.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualificationPlanV1 {
    schema_id: String,
    schema_version: String,
    lock_path: PathBuf,
    required_languages: Vec<String>,
    cases: Vec<QualificationCaseV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualificationCaseV1 {
    case_id: String,
    resource_id: String,
    language_id: String,
    provider_id: String,
    search: QualificationSearchV1,
    query: QualificationQueryV1,
    zero_match_terms: Vec<String>,
    required_telemetry_events: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualificationSearchV1 {
    method: String,
    terms: Vec<String>,
    view: String,
    minimum_candidates: usize,
    maximum_resident_micros: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualificationQueryV1 {
    selector_strategy: String,
    owner_view: String,
    projection_scope: String,
    maximum_resident_micros: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QualificationReceiptV2 {
    schema_id: &'static str,
    schema_version: &'static str,
    plan_digest: String,
    lock_digest: String,
    qualified_case_count: usize,
    cases: Vec<QualificationCaseReceiptV2>,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QualificationCaseReceiptV2 {
    case_id: String,
    resource_id: String,
    language_id: String,
    provider_id: String,
    revision: String,
    git_tree: String,
    generation_digest: String,
    root_digest: String,
    search_operation_id: String,
    search_elapsed_micros: u64,
    candidate_count: usize,
    selector: String,
    query_operation_id: String,
    query_elapsed_micros: u64,
    merkle_owner_path: String,
    merkle_source_blob_digest: String,
    merkle_owner_subtree_digest: String,
    merkle_proof_digest: String,
    merkle_proof_step_count: usize,
    zero_match_operation_id: String,
    telemetry_enqueue_accepted: Vec<bool>,
    runtime_ecosystem: &'static str,
    search_read_mode: &'static str,
    search_read_work_counters:
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters,
    exact_read_mode: &'static str,
    exact_read_work_counters:
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters,
    status: &'static str,
}

struct ResidentSearchOutcome {
    elapsed_micros: u64,
    candidate_count: usize,
    selectors: Vec<String>,
    owner_paths: Vec<String>,
}

pub(super) struct QualifyArgs {
    plan_path: PathBuf,
    json: bool,
}

fn publish_qualification_receipt(state_home: &Path, encoded: &str) -> Result<PathBuf, String> {
    let receipt_path = state_home
        .join("runtime")
        .join("live-corpus")
        .join("search-query-qualification.v2.json");
    let parent = receipt_path
        .parent()
        .ok_or_else(|| "Live Corpus qualification receipt has no parent".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Live Corpus qualification receipt directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".search-query-qualification.v2.{}.tmp",
        std::process::id()
    ));
    std::fs::write(&temporary, encoded).map_err(|error| {
        format!(
            "write Live Corpus qualification receipt {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &receipt_path).map_err(|error| {
        format!(
            "publish Live Corpus qualification receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    Ok(receipt_path)
}

pub(super) async fn run(args: &[String]) -> Result<(), String> {
    let args = parse_args(args)?;
    let plan_bytes = std::fs::read(&args.plan_path).map_err(|error| {
        format!(
            "failed to read Live Corpus qualification plan {}: {error}",
            args.plan_path.display()
        )
    })?;
    let plan = serde_json::from_slice::<QualificationPlanV1>(&plan_bytes)
        .map_err(|error| format!("failed to decode Live Corpus qualification plan: {error}"))?;
    validate_plan(&plan)?;
    let lock_bytes = std::fs::read(&plan.lock_path).map_err(|error| {
        format!(
            "failed to read Live Corpus lock {}: {error}",
            plan.lock_path.display()
        )
    })?;
    let lock = super::load_lock(&plan.lock_path)?;
    let state_home = super::resolve_state_home()?;
    let endpoint = agent_semantic_client_db::read_runtime_server_endpoint(&state_home)?
        .ok_or_else(|| "Live Corpus qualification requires a healthy Runtime Server".to_owned())?;

    let mut receipts = Vec::with_capacity(plan.cases.len());
    for case in plan.cases {
        let corpus = super::unique_resource(&lock.corpora, &case.resource_id)?;
        if corpus.language.as_str() != case.language_id || corpus.provider_id != case.provider_id {
            return Err(format!(
                "Live Corpus qualification identity drift: resource={} planLanguage={} lockLanguage={} planProvider={} lockProvider={}",
                case.resource_id,
                case.language_id,
                corpus.language,
                case.provider_id,
                corpus.provider_id
            ));
        }
        let repository = super::live_corpus_git_repository_paths(&state_home, &corpus.git.remote)?;
        let checkout_path = repository
            .repository_dir
            .join("checkouts")
            .join(&corpus.git.revision);
        let current_pointer = state_home
            .join("artifacts")
            .join("live-corpus")
            .join("v1")
            .join("by-resource")
            .join(&case.resource_id)
            .join("current");
        let artifact_dir = current_pointer.canonicalize().map_err(|error| {
            format!(
                "Live Corpus qualification requires a prepublished immutable artifact: resource={} pointer={} error={error}",
                case.resource_id,
                current_pointer.display()
            )
        })?;
        let qualification_path = artifact_dir.join("qualification.json");
        let qualification = serde_json::from_slice::<super::LiveCorpusQualificationV1>(
            &std::fs::read(&qualification_path).map_err(|error| {
                format!(
                    "failed to read Live Corpus immutable qualification {}: {error}",
                    qualification_path.display()
                )
            })?,
        )
        .map_err(|error| {
            format!(
                "failed to decode Live Corpus immutable qualification {}: {error}",
                qualification_path.display()
            )
        })?;
        let artifact_digest = artifact_dir
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| {
                format!(
                    "Live Corpus immutable artifact has no digest identity: {}",
                    artifact_dir.display()
                )
            })?;
        if qualification.schema_id != "agent.semantic-protocols.live-corpus-artifact-qualification"
            || qualification.schema_version != "1"
            || qualification.status != "qualified"
            || !qualification.clean
            || qualification.artifact_digest != artifact_digest
            || qualification.head_revision != corpus.git.revision
            || Path::new(&qualification.source_path) != checkout_path
        {
            return Err(format!(
                "Live Corpus immutable artifact identity drift: case={} artifact={}",
                case.case_id,
                artifact_dir.display()
            ));
        }
        let qualified = qualify_case(
            &endpoint,
            &checkout_path,
            case,
            qualification.head_revision,
            qualification.git_tree,
        )
        .await?;
        if qualified.root_digest != qualification.source_merkle_root {
            return Err(format!(
                "Live Corpus resident generation root does not match immutable artifact: case={} artifactRoot={} residentRoot={}",
                qualified.case_id, qualification.source_merkle_root, qualified.root_digest
            ));
        }
        receipts.push(qualified);
    }
    let receipt = QualificationReceiptV2 {
        schema_id: "agent.semantic-protocols.live-corpus-search-query-qualification-receipt.v2",
        schema_version: "2",
        plan_digest: super::live_corpus_lock_digest(&plan_bytes),
        lock_digest: super::live_corpus_lock_digest(&lock_bytes),
        qualified_case_count: receipts.len(),
        cases: receipts,
        status: "qualified",
    };
    let encoded = serde_json::to_string(&receipt)
        .map_err(|error| format!("encode Live Corpus qualification receipt: {error}"))?;
    let receipt_path = publish_qualification_receipt(&state_home, &encoded)?;
    if args.json {
        println!("{encoded}");
    } else {
        println!(
            "[live-corpus-qualification] cases={} planDigest={} lockDigest={} receipt={} status=qualified",
            receipt.qualified_case_count,
            receipt.plan_digest,
            receipt.lock_digest,
            receipt_path.display()
        );
    }
    Ok(())
}

async fn qualify_case(
    endpoint: &agent_semantic_client_db::RuntimeServerEndpoint,
    project_root: &Path,
    case: QualificationCaseV1,
    revision: String,
    git_tree: String,
) -> Result<QualificationCaseReceiptV2, String> {
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string();
    let session = agent_semantic_client_db::WorkspaceDbIpcSession::for_runtime_server_client(
        endpoint,
        workspace_identity.clone(),
        project_root.to_path_buf(),
    );
    let resident_generation_pointer = session.runtime_generation_pointer_path().ok_or_else(|| {
        format!(
            "Live Corpus resident generation pointer is missing: case={} reasonKind=runtime-generation-not-ready",
            case.case_id
        )
    })?;
    let resident_read =
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient::open(
            &resident_generation_pointer,
            project_root,
        )
        .await?;
    let required_generation = tokio::time::timeout(
        std::time::Duration::from_millis(10),
        session.require_runtime_generation(),
    )
    .await
    .map_err(|_| {
        format!(
            "Live Corpus resident generation authority exceeded 10ms: case={}",
            case.case_id
        )
    })??;
    let required_commit = required_generation.commit.ok_or_else(|| {
        format!(
            "Live Corpus resident generation has no immutable commit: case={}",
            case.case_id
        )
    })?;
    let resident_generation_digest = resident_read.generation_digest();
    let resident_root_digest = resident_read.root_digest();
    if resident_generation_digest != required_commit.generation_digest
        || resident_root_digest != required_commit.source_root_digest
    {
        return Err(format!(
            "Live Corpus resident authority is stale before search: case={} reasonKind=stale-generation requiredGeneration={} residentGeneration={} requiredRoot={} residentRoot={} candidates=[]",
            case.case_id,
            required_commit.generation_digest,
            resident_generation_digest,
            required_commit.source_root_digest,
            resident_root_digest,
        ));
    }

    let language_id = LanguageId::from(case.language_id.as_str());
    let search_operation_id = format!("live-corpus-search-{}", case.case_id);
    let search = resident_search(
        &resident_read,
        &language_id,
        &case.search.method,
        &case.search.view,
        &case.search.terms,
    )?;
    if search.elapsed_micros > case.search.maximum_resident_micros {
        return Err(format!(
            "Live Corpus resident search exceeded budget: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, search.elapsed_micros, case.search.maximum_resident_micros
        ));
    }
    if search.candidate_count < case.search.minimum_candidates {
        return Err(format!(
            "Live Corpus resident search returned too few candidates: case={} candidates={} minimum={}",
            case.case_id, search.candidate_count, case.search.minimum_candidates
        ));
    }
    let selector = search.selectors.first().cloned().ok_or_else(|| {
        format!(
            "Live Corpus resident search returned no parser-owned selector: case={}",
            case.case_id
        )
    })?;
    let search_telemetry_enqueued = resident_read.try_record_read_observation(
        "runtime-resident-search",
        "resident-read",
        &search_operation_id,
        Some(&language_id),
        "source-index",
        search.elapsed_micros,
        case.search.maximum_resident_micros,
        "within-budget",
    );
    let query_started = tokio::time::Instant::now();
    let query = resident_read.read_runtime_selector(ExactProjectionKind::Source, &selector)?;
    let query_elapsed_micros = query_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    if query_elapsed_micros > case.query.maximum_resident_micros {
        return Err(format!(
            "Live Corpus exact query exceeded budget: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, query_elapsed_micros, case.query.maximum_resident_micros
        ));
    }
    let (generation_digest, root_digest) = match query {
        WorkspaceRuntimeSelectorRead::Projection {
            generation_digest,
            root_digest,
            ..
        } => (generation_digest, root_digest),
        other => {
            return Err(format!(
                "Live Corpus exact query did not return a resident projection: case={} state={other:?}",
                case.case_id
            ));
        }
    };
    if generation_digest != resident_generation_digest || root_digest != resident_root_digest {
        return Err(format!(
            "Live Corpus resident generation authority drifted during query: case={} reasonKind=stale-generation residentGeneration={} queryGeneration={} residentRoot={} queryRoot={} candidates=[]",
            case.case_id,
            resident_generation_digest,
            generation_digest,
            resident_root_digest,
            root_digest
        ));
    }
    let merkle_owner_path = search.owner_paths.first().ok_or_else(|| {
        format!(
            "runtime-resident-search-owner-path-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_receipt = resident_read.qualify_merkle_owner_proof(
        agent_semantic_client_db::runtime_merkle_owner_proof_qualification::RuntimeMerkleOwnerProofEvidenceLayer::LiveCorpus,
        case.case_id.clone(),
        Some(case.resource_id.clone()),
        case.language_id.clone(),
        case.provider_id.clone(),
        merkle_owner_path,
        selector.clone(),
    )?;
    if merkle_receipt.generation_digest.as_deref()
        != Some(required_commit.generation_digest.as_str())
        || merkle_receipt.root_digest.as_deref()
            != Some(required_commit.source_root_digest.as_str())
        || merkle_receipt.owner_path.as_deref() != Some(merkle_owner_path.as_str())
        || merkle_receipt.proof_step_count == Some(0)
    {
        return Err(format!(
            "runtime-resident-merkle-authority-mismatch: case={} ownerPath={merkle_owner_path}",
            case.case_id
        ));
    }
    let merkle_source_blob_digest = merkle_receipt.owner_content_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-content-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_owner_subtree_digest = merkle_receipt.owner_subtree_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-subtree-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_proof_digest = merkle_receipt.proof_digest.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-proof-digest-missing: case={}",
            case.case_id
        )
    })?;
    let merkle_proof_step_count = merkle_receipt.proof_step_count.ok_or_else(|| {
        format!(
            "runtime-resident-merkle-proof-steps-missing: case={}",
            case.case_id
        )
    })?;
    let query_operation_id = format!("live-corpus-query-{}", case.case_id);
    let query_telemetry_enqueued = resident_read.try_record_read_observation(
        "runtime-exact-projection",
        "resident-read",
        &query_operation_id,
        Some(&language_id),
        "source",
        query_elapsed_micros,
        case.query.maximum_resident_micros,
        "within-budget",
    );

    let zero_match_operation_id = format!("live-corpus-zero-match-{}", case.case_id);
    let zero_match = resident_search(
        &resident_read,
        &language_id,
        "lexical",
        "seeds",
        &case.zero_match_terms,
    )?;
    if zero_match.candidate_count != 0 {
        return Err(format!(
            "Live Corpus zero-match query returned candidates: case={} candidates={}",
            case.case_id, zero_match.candidate_count
        ));
    }
    if zero_match.elapsed_micros > case.search.maximum_resident_micros {
        return Err(format!(
            "runtime-resident-zero-match-budget-exceeded: case={} elapsedMicros={} budgetMicros={}",
            case.case_id, zero_match.elapsed_micros, case.search.maximum_resident_micros
        ));
    }
    let zero_match_telemetry_enqueued = resident_read.try_record_read_observation(
        "runtime-resident-search",
        "resident-read",
        &zero_match_operation_id,
        Some(&language_id),
        "source-index",
        zero_match.elapsed_micros,
        case.search.maximum_resident_micros,
        "within-budget",
    );
    let telemetry_enqueue_accepted = vec![
        search_telemetry_enqueued,
        query_telemetry_enqueued,
        zero_match_telemetry_enqueued,
    ];
    Ok(QualificationCaseReceiptV2 {
        case_id: case.case_id,
        resource_id: case.resource_id,
        language_id: case.language_id,
        provider_id: case.provider_id,
        revision,
        git_tree,
        generation_digest,
        root_digest,
        search_operation_id,
        search_elapsed_micros: search.elapsed_micros,
        candidate_count: search.candidate_count,
        selector,
        query_operation_id,
        query_elapsed_micros,
        merkle_owner_path: merkle_owner_path.clone(),
        merkle_source_blob_digest,
        merkle_owner_subtree_digest,
        merkle_proof_digest,
        merkle_proof_step_count,
        zero_match_operation_id,
        telemetry_enqueue_accepted,
        runtime_ecosystem: "tokio",
        search_read_mode: "synchronous-mmap",
        search_read_work_counters: resident_read.work_counters(),
        exact_read_mode: "synchronous-mmap",
        exact_read_work_counters: resident_read.work_counters(),
        status: "qualified",
    })
}

fn resident_search(
    resident_read: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    language_id: &agent_semantic_client_core::LanguageId,
    method: &str,
    view: &str,
    terms: &[String],
) -> Result<ResidentSearchOutcome, String> {
    const CANDIDATE_LIMIT: u32 = 64;
    if method != "lexical" || view != "seeds" {
        return Err(format!(
            "runtime-resident-search-plan-unsupported: method={method} view={view}"
        ));
    }
    let query = terms.join(" ");
    let started = std::time::Instant::now();
    let lookup = resident_read.read_source_index(&query, Some(language_id), CANDIDATE_LIMIT)?;
    let elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let resident_root = resident_read.root_digest();
    let observed_root = lookup
        .source_snapshot
        .as_ref()
        .ok_or_else(|| "runtime-resident-search-source-snapshot-missing".to_owned())?
        .root_digest
        .to_string();
    if observed_root != resident_root {
        return Err(format!(
            "runtime-resident-search-root-mismatch: expected={resident_root} observed={observed_root}"
        ));
    }
    let mut selectors = Vec::new();
    let mut owner_paths = Vec::new();
    for candidate in &lookup.candidates {
        if let Some(projection) = &candidate.selector_projection {
            selectors.push(projection.proof.structural_selector().to_owned());
            owner_paths.push(projection.proof.owner_path().to_owned());
        }
    }
    Ok(ResidentSearchOutcome {
        elapsed_micros,
        candidate_count: lookup.candidates.len(),
        selectors,
        owner_paths,
    })
}

fn validate_plan(plan: &QualificationPlanV1) -> Result<(), String> {
    if plan.schema_id != "agent.semantic-protocols.live-corpus-search-query-qualification-plan.v1"
        || plan.schema_version != "1"
    {
        return Err("unsupported Live Corpus qualification plan schema".to_owned());
    }
    if plan.cases.is_empty() || plan.required_languages.is_empty() {
        return Err("Live Corpus qualification plan is empty".to_owned());
    }
    for case in &plan.cases {
        if case.query.selector_strategy != "first-ranked-parser-owned"
            || case.query.owner_view != "items"
            || case.query.projection_scope != "live-corpus"
            || case.search.maximum_resident_micros > 1_000
            || case.query.maximum_resident_micros > 1_000
            || case.required_telemetry_events
                != [
                    "runtime_resident_search_terminal",
                    "runtime_exact_projection_terminal",
                ]
        {
            return Err(format!(
                "Live Corpus qualification case violates the resident contract: case={}",
                case.case_id
            ));
        }
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<QualifyArgs, String> {
    let mut plan_path = PathBuf::from(DEFAULT_PLAN_PATH);
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                index += 1;
                plan_path = PathBuf::from(args.get(index).ok_or_else(|| {
                    "live-corpus qualify requires a path after --plan".to_owned()
                })?);
            }
            "--json" => json = true,
            option => return Err(format!("unknown live-corpus qualify option: {option}")),
        }
        index += 1;
    }
    Ok(QualifyArgs { plan_path, json })
}
