//! Live Corpus qualification runner over the shared ASP Client application boundary.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use super::client_protocol::qualify_public_client_case;
use crate::command::live_corpus::{
    LiveCorpusQualification, live_corpus_git_repository_paths, live_corpus_lock_digest, load_lock,
    resolve_state_home, unique_resource,
};

const DEFAULT_PLAN_PATH: &str = "benchmarks/live-corpus-search-query-qualification.json";

#[derive(Debug)]
pub(super) struct QualifyArgs {
    plan_path: PathBuf,
    resource_id: Option<String>,
    json: bool,
}

struct PreparedCase {
    case: QualificationCase,
    checkout_path: PathBuf,
    qualification: LiveCorpusQualification,
}

struct PreparedRun {
    args: QualifyArgs,
    plan_bytes: Vec<u8>,
    lock_bytes: Vec<u8>,
    state_home: PathBuf,
    cases: Vec<PreparedCase>,
}

fn publish_qualification_receipt(state_home: &Path, encoded: &str) -> Result<PathBuf, String> {
    let receipt_path = state_home
        .join("runtime")
        .join("live-corpus")
        .join("search-query-qualification.json");
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
        ".search-query-qualification.{}.tmp",
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

pub(crate) async fn run(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    let prepared = tokio::task::spawn_blocking(move || prepare_run(&args))
        .await
        .map_err(|error| format!("prepare Live Corpus qualification task: {error}"))??;
    let PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        state_home,
        cases,
    } = prepared;
    let mut receipts = Vec::with_capacity(cases.len());
    for prepared_case in cases {
        let client = agent_semantic_client::RuntimeLanguageCommandClient;
        let checkout_path = prepared_case.checkout_path.clone();
        let source_merkle_root = prepared_case.qualification.source_merkle_root.clone();
        let qualified = qualify_case(
            &client,
            &checkout_path,
            prepared_case.case,
            prepared_case.qualification.head_revision,
            prepared_case.qualification.git_tree,
        )
        .await?;
        if qualified.root_digest != source_merkle_root {
            return Err(format!(
                "Live Corpus public route root does not match immutable artifact: case={} artifactRoot={} publicRoot={}",
                qualified.case_id, source_merkle_root, qualified.root_digest
            ));
        }
        receipts.push(qualified);
    }
    let receipt = QualificationReceipt {
        schema_id: "agent.semantic-protocols.live-corpus-search-query-qualification-receipt",
        schema_version: "1",
        plan_digest: live_corpus_lock_digest(&plan_bytes),
        lock_digest: live_corpus_lock_digest(&lock_bytes),
        client_protocol: ClientProtocolReceipt {
            protocol_id: "agent.semantic-protocols.client",
            protocol_version: "1",
            transport: "grpc-tokio-streams",
            application_api: "agent_semantic_client::AspClient::dispatch",
            routes: ["search", "query"],
            terminal_outcomes: ["queued", "building", "ready", "failed", "cancelled"],
            qualified_case_count: receipts.len(),
        },
        qualified_case_count: receipts.len(),
        cases: receipts,
        status: "qualified",
    };
    let encoded = serde_json::to_string(&receipt)
        .map_err(|error| format!("encode Live Corpus qualification receipt: {error}"))?;
    let publish_state_home = state_home.clone();
    let publish_encoded = encoded.clone();
    let receipt_path = tokio::task::spawn_blocking(move || {
        publish_qualification_receipt(&publish_state_home, &publish_encoded)
    })
    .await
    .map_err(|error| format!("publish Live Corpus qualification task: {error}"))??;
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

fn prepare_run(args: &[String]) -> Result<PreparedRun, String> {
    let args = parse_args(args)?;
    let plan_bytes = std::fs::read(&args.plan_path).map_err(|error| {
        format!(
            "failed to read Live Corpus qualification plan {}: {error}",
            args.plan_path.display()
        )
    })?;
    let plan = serde_json::from_slice::<QualificationPlan>(&plan_bytes)
        .map_err(|error| format!("failed to decode Live Corpus qualification plan: {error}"))?;
    validate_plan(&plan)?;
    let lock_bytes = std::fs::read(&plan.lock_path).map_err(|error| {
        format!(
            "failed to read Live Corpus lock {}: {error}",
            plan.lock_path.display()
        )
    })?;
    let lock = load_lock(&plan.lock_path)?;
    let state_home = resolve_state_home()?;

    let cases = select_qualification_cases(plan.cases, args.resource_id.as_deref())?;
    let mut prepared_cases = Vec::with_capacity(cases.len());
    for case in cases {
        let corpus = unique_resource(&lock.corpora, &case.resource_id)?;
        if corpus.scenario_id != case.scenario_id
            || corpus.language.as_str() != case.language_id
            || corpus.provider_id != case.provider_id
        {
            return Err(format!(
                "Live Corpus qualification identity drift: resource={} planScenario={} lockScenario={} planLanguage={} lockLanguage={} planProvider={} lockProvider={}",
                case.resource_id,
                case.scenario_id,
                corpus.scenario_id,
                case.language_id,
                corpus.language,
                case.provider_id,
                corpus.provider_id
            ));
        }
        let repository = live_corpus_git_repository_paths(&state_home, &corpus.git.remote)?;
        let checkout_path = repository
            .repository_dir
            .join("checkouts")
            .join(&corpus.git.revision);
        let current_pointer = state_home
            .join("artifacts")
            .join("live-corpus")
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
        let qualification = serde_json::from_slice::<LiveCorpusQualification>(
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
        prepared_cases.push(PreparedCase {
            case,
            checkout_path,
            qualification,
        });
    }
    Ok(PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        state_home,
        cases: prepared_cases,
    })
}

async fn qualify_case<C: agent_semantic_client::LanguageCommandClient>(
    client: &C,
    project_root: &std::path::Path,
    case: QualificationCase,
    revision: String,
    git_tree: String,
) -> Result<QualificationCaseReceipt, String> {
    let evidence = qualify_public_client_case(client, project_root, &case).await?;
    let selector = evidence.search.selectors.first().cloned().ok_or_else(|| {
        format!(
            "Live Corpus public search selector missing: case={}",
            case.case_id
        )
    })?;
    for query in [&evidence.source, &evidence.callable_skeleton] {
        if query.generation_digest != evidence.search.generation_digest
            || query.root_digest != evidence.search.root_digest
            || query.provider_id != case.provider_id
        {
            return Err(format!(
                "Live Corpus public route authority drift: case={} searchGeneration={} queryGeneration={} searchRoot={} queryRoot={}",
                case.case_id,
                evidence.search.generation_digest,
                query.generation_digest,
                evidence.search.root_digest,
                query.root_digest
            ));
        }
    }
    if evidence.zero_match.generation_digest != evidence.search.generation_digest
        || evidence.zero_match.root_digest != evidence.search.root_digest
    {
        return Err(format!(
            "Live Corpus public zero-match route crossed generation authority: case={}",
            case.case_id
        ));
    }
    Ok(QualificationCaseReceipt {
        case_id: case.case_id,
        resource_id: case.resource_id,
        scenario_id: case.scenario_id,
        language_id: case.language_id,
        provider_id: case.provider_id,
        revision,
        git_tree,
        generation_digest: evidence.search.generation_digest,
        root_digest: evidence.search.root_digest,
        search_operation_id: evidence.search.operation_id,
        search_elapsed_micros: evidence.search.elapsed_micros,
        candidate_count: evidence.search.candidate_count,
        selector,
        query_operation_id: evidence.source.operation_id,
        query_elapsed_micros: evidence.source.elapsed_micros,
        callable_skeleton_operation_id: evidence.callable_skeleton.operation_id,
        callable_skeleton_elapsed_micros: evidence.callable_skeleton.elapsed_micros,
        zero_match_operation_id: evidence.zero_match.operation_id,
        route: "public-typed-asp-client",
        search_terminal: "ready",
        query_terminal: "ready",
        callable_skeleton_terminal: "ready",
        zero_match_terminal: "ready",
        status: "qualified",
    })
}

fn validate_plan(plan: &QualificationPlan) -> Result<(), String> {
    if plan.schema_id != "agent.semantic-protocols.live-corpus-search-query-qualification-plan"
        || plan.schema_version != "1"
    {
        return Err("unsupported Live Corpus qualification plan schema".to_owned());
    }
    if plan.cases.is_empty() || plan.required_languages.is_empty() {
        return Err("Live Corpus qualification plan is empty".to_owned());
    }
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registered = agent_semantic_schema_manager::SchemaManager::new(workspace_root)
        .registered_language_profiles()?
        .into_iter()
        .map(|profile| profile.language_id)
        .collect::<BTreeSet<_>>();
    let required = plan
        .required_languages
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required != registered {
        return Err(format!(
            "Live Corpus required languages must equal SchemaManager registered profiles: required={required:?} registered={registered:?}"
        ));
    }
    let case_languages = plan
        .cases
        .iter()
        .map(|case| case.language_id.as_str())
        .collect::<BTreeSet<_>>();
    for profile in &registered {
        if !case_languages.contains(profile.as_str()) {
            return Err(format!(
                "Live Corpus has no public-route case for registered language profile: language={profile}"
            ));
        }
    }
    for case in &plan.cases {
        if !registered.contains(&case.language_id) {
            return Err(format!(
                "Live Corpus case language is not registered by SchemaManager: case={} language={}",
                case.case_id, case.language_id
            ));
        }
        if case.query.selector_strategy != "first-ranked-parser-owned"
            || case.search.method != "lexical"
            || case.search.view != "seeds"
        {
            return Err(format!(
                "Live Corpus case is not an ordinary public search/query route: case={}",
                case.case_id
            ));
        }
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<QualifyArgs, String> {
    let mut plan_path = PathBuf::from(DEFAULT_PLAN_PATH);
    let mut resource_id = None;
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
            "--resource" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    "live-corpus qualify requires a resource id after --resource".to_owned()
                })?;
                if value.trim().is_empty() {
                    return Err("live-corpus qualify resource id must be non-empty text".to_owned());
                }
                if resource_id.replace(value.clone()).is_some() {
                    return Err(
                        "live-corpus qualify accepts exactly one --resource option".to_owned()
                    );
                }
            }
            option => return Err(format!("unknown live-corpus qualify option: {option}")),
        }
        index += 1;
    }
    Ok(QualifyArgs {
        plan_path,
        resource_id,
        json,
    })
}

fn select_qualification_cases(
    cases: Vec<QualificationCase>,
    resource_id: Option<&str>,
) -> Result<Vec<QualificationCase>, String> {
    let Some(resource_id) = resource_id else {
        return Ok(cases);
    };
    let selected = cases
        .into_iter()
        .filter(|case| case.resource_id == resource_id)
        .collect::<Vec<_>>();
    match selected.len() {
        0 => Err(format!(
            "Live Corpus qualification resource was not found in plan: resource={resource_id}"
        )),
        1 => Ok(selected),
        count => Err(format!(
            "Live Corpus qualification resource is duplicated in plan: resource={resource_id} count={count}"
        )),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/command/live_corpus_qualification.rs"]
mod tests;
use super::contract::{
    ClientProtocolReceipt, QualificationCase, QualificationCaseReceipt, QualificationPlan,
    QualificationReceipt,
};
