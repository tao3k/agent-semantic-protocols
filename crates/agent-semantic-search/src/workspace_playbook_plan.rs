//! Deterministic workspace planner for the sole public Search playbook.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{SearchPlaybookRequest, canonical_blake3_digest};

pub const WORKSPACE_SEARCH_PLAYBOOK_PLAN_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-search-playbook-plan";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProvider {
    pub language_id: String,
    pub provider_id: String,
    pub source_extensions: Vec<String>,
    pub search_supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchPlanBinding {
    pub project_id: String,
    pub workspace_id: String,
    pub content_generation_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSearchStageKind {
    RgAcquisition,
    ProviderNativeSyntax,
    TantivyLexical,
    PythonGraph,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSearchStagePolicy {
    ContentRequired,
    AcceleratorIfReady,
    IntentRequired,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchStagePlan {
    pub stage: WorkspaceSearchStageKind,
    pub authority: String,
    pub policy: WorkspaceSearchStagePolicy,
    pub depends_on: Vec<WorkspaceSearchStageKind>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchRouteBudget {
    pub max_results: u32,
    pub max_graph_nodes: u32,
    pub deadline_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchPlaybookRoute {
    pub language_id: String,
    pub provider_id: String,
    pub generation_digest: String,
    pub extensions: Vec<String>,
    pub operation: String,
    pub normalized_terms: Vec<String>,
    pub selectors: Vec<String>,
    pub stages: Vec<WorkspaceSearchStagePlan>,
    pub budget: WorkspaceSearchRouteBudget,
    pub depends_on: Vec<String>,
    pub public_command: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSearchSkipReason {
    LanguageNotSelected,
    ProviderUnavailable,
    GenerationUnavailable,
    SourceScopeEmpty,
    IntentNotSupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchSkippedLanguage {
    pub language_id: String,
    pub reason_kind: WorkspaceSearchSkipReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchWarmWork {
    pub filesystem_read_count: u64,
    pub provider_process_count: u64,
    pub socket_discovery_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchPlaybookPlan {
    pub schema_id: String,
    pub schema_version: String,
    pub project_id: String,
    pub workspace_id: String,
    pub content_generation_digest: String,
    pub intent: String,
    pub query: String,
    pub normalized_terms: Vec<String>,
    pub language_constraint: Option<String>,
    pub max_concurrency: u32,
    pub routes: Vec<WorkspaceSearchPlaybookRoute>,
    pub skipped_languages: Vec<WorkspaceSearchSkippedLanguage>,
    pub work: WorkspaceSearchWarmWork,
}

pub fn build_workspace_search_playbook_plan(
    request: &SearchPlaybookRequest,
    binding: WorkspaceSearchPlanBinding,
    providers: impl IntoIterator<Item = WorkspaceSearchProvider>,
) -> Result<WorkspaceSearchPlaybookPlan, String> {
    request.validate()?;
    if binding.project_id.trim().is_empty() || binding.workspace_id.trim().is_empty() {
        return Err("workspace playbook requires ProjectId and WorkspaceId".to_owned());
    }
    canonical_blake3_digest(&binding.content_generation_digest)?;
    let normalized_terms = normalize_search_terms(&request.query);
    if normalized_terms.is_empty() {
        return Err("search playbook query has no searchable terms".to_owned());
    }

    let mut providers = providers.into_iter().collect::<Vec<_>>();
    providers.sort_by(|left, right| {
        left.language_id
            .cmp(&right.language_id)
            .then_with(|| left.provider_id.cmp(&right.provider_id))
    });
    if providers
        .windows(2)
        .any(|pair| pair[0].language_id == pair[1].language_id)
    {
        return Err("workspace playbook requires one provider authority per language".to_owned());
    }
    if let Some(language) = request.language.as_deref()
        && !providers
            .iter()
            .any(|provider| provider.language_id == language)
    {
        return Err(format!(
            "workspace playbook has no admitted provider for language `{language}`"
        ));
    }

    let mut routes = Vec::new();
    let mut skipped_languages = Vec::new();
    for mut provider in providers {
        if request
            .language
            .as_ref()
            .is_some_and(|language| language != &provider.language_id)
        {
            skipped_languages.push(WorkspaceSearchSkippedLanguage {
                language_id: provider.language_id,
                reason_kind: WorkspaceSearchSkipReason::LanguageNotSelected,
            });
            continue;
        }
        if !provider.search_supported {
            skipped_languages.push(WorkspaceSearchSkippedLanguage {
                language_id: provider.language_id,
                reason_kind: WorkspaceSearchSkipReason::IntentNotSupported,
            });
            continue;
        }
        provider.source_extensions.sort_unstable();
        provider.source_extensions.dedup();
        if provider.source_extensions.is_empty() {
            skipped_languages.push(WorkspaceSearchSkippedLanguage {
                language_id: provider.language_id,
                reason_kind: WorkspaceSearchSkipReason::SourceScopeEmpty,
            });
            continue;
        }
        let language_id = provider.language_id;
        routes.push(WorkspaceSearchPlaybookRoute {
            language_id: language_id.clone(),
            provider_id: provider.provider_id,
            generation_digest: binding.content_generation_digest.clone(),
            extensions: provider.source_extensions,
            operation: "search".to_owned(),
            normalized_terms: normalized_terms.clone(),
            selectors: Vec::new(),
            stages: stage_plan(&request.intent, &language_id),
            budget: WorkspaceSearchRouteBudget {
                max_results: request.max_owners,
                max_graph_nodes: request.max_owners.saturating_mul(4),
                deadline_millis: request.deadline_ms.min(1_000),
            },
            depends_on: Vec::new(),
            public_command: vec![
                "asp".to_owned(),
                language_id,
                "search".to_owned(),
                "playbook".to_owned(),
                request.query.clone(),
                "--intent".to_owned(),
                request.intent.clone(),
                "--scope".to_owned(),
                request.scope.clone(),
                "--coverage".to_owned(),
                request.coverage.clone(),
                "--workspace".to_owned(),
                request.workspace.clone(),
            ],
        });
    }

    Ok(WorkspaceSearchPlaybookPlan {
        schema_id: WORKSPACE_SEARCH_PLAYBOOK_PLAN_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        project_id: binding.project_id,
        workspace_id: binding.workspace_id,
        content_generation_digest: binding.content_generation_digest,
        intent: request.intent.clone(),
        query: request.query.clone(),
        normalized_terms,
        language_constraint: request.language.clone(),
        max_concurrency: u32::try_from(routes.len().clamp(1, 8)).unwrap_or(8),
        routes,
        skipped_languages,
        work: WorkspaceSearchWarmWork {
            filesystem_read_count: 0,
            provider_process_count: 0,
            socket_discovery_count: 0,
        },
    })
}

fn stage_plan(intent: &str, language_id: &str) -> Vec<WorkspaceSearchStagePlan> {
    vec![
        WorkspaceSearchStagePlan {
            stage: WorkspaceSearchStageKind::RgAcquisition,
            authority: "asp-server".to_owned(),
            policy: WorkspaceSearchStagePolicy::ContentRequired,
            depends_on: Vec::new(),
        },
        WorkspaceSearchStagePlan {
            stage: WorkspaceSearchStageKind::ProviderNativeSyntax,
            authority: format!("asp-{language_id}"),
            policy: WorkspaceSearchStagePolicy::ContentRequired,
            depends_on: vec![WorkspaceSearchStageKind::RgAcquisition],
        },
        WorkspaceSearchStagePlan {
            stage: WorkspaceSearchStageKind::TantivyLexical,
            authority: "asp-server".to_owned(),
            policy: WorkspaceSearchStagePolicy::AcceleratorIfReady,
            depends_on: vec![WorkspaceSearchStageKind::ProviderNativeSyntax],
        },
        WorkspaceSearchStagePlan {
            stage: WorkspaceSearchStageKind::PythonGraph,
            authority: "asp-python-graphs".to_owned(),
            policy: if intent == "relationship" {
                WorkspaceSearchStagePolicy::IntentRequired
            } else {
                WorkspaceSearchStagePolicy::Skipped
            },
            depends_on: vec![WorkspaceSearchStageKind::ProviderNativeSyntax],
        },
    ]
}

fn normalize_search_terms(query: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for term in query
        .split(|character: char| {
            !character.is_alphanumeric() && !matches!(character, '_' | '-' | '.')
        })
        .filter(|term| !term.is_empty())
    {
        terms.insert(term.to_lowercase());
    }
    terms.into_iter().collect()
}
