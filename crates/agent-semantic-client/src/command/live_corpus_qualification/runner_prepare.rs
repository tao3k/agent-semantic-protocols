// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Admits Live Corpus plans and binds them to immutable corpus artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::client_protocol::ResidentSearchLatencyBudget;
use super::contract::{
    AgentOrgTopologyScenario, AgentOrgTopologyScenarioSuite, QualificationCase, QualificationPlan,
};
use super::query_protocol::validate_workspace_query_set_scheme_template;
use super::runner::{DEFAULT_TOPOLOGY_PLAN_PATH, PreparedCase, PreparedRun};
use super::runner_contract::{parse_args, select_qualification_cases, validate_plan};
use crate::command::live_corpus::{
    LiveCorpusQualification, live_corpus_git_repository_paths, load_lock, unique_resource,
};

pub(super) fn artifact_current_pointer(state_home: &Path, resource_id: &str) -> PathBuf {
    agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .resources()
        .live_corpus()
        .join("artifacts")
        .join("by-resource")
        .join(resource_id)
        .join("current")
}

pub(super) fn prepare_run(
    args: &[String],
    runtime_state_home: PathBuf,
    resource_state_home: PathBuf,
) -> Result<PreparedRun, String> {
    let mut args = parse_args(args)?;
    args.plan_path = qualification_input_path(&args.plan_path);
    let plan_bytes = std::fs::read(&args.plan_path).map_err(|error| {
        format!(
            "failed to read Live Corpus qualification plan {}: {error}",
            args.plan_path.display()
        )
    })?;
    let plan_source = std::str::from_utf8(&plan_bytes)
        .map_err(|error| format!("Live Corpus Scheme scenario suite is not UTF-8: {error}"))?;
    let plan = toml::from_str::<QualificationPlan>(plan_source)
        .map_err(|error| format!("failed to decode Live Corpus Scheme scenario suite: {error}"))?;
    validate_plan(&plan)?;
    let topology_plan_path = qualification_input_path(Path::new(DEFAULT_TOPOLOGY_PLAN_PATH));
    let topology_plan_source = std::fs::read_to_string(&topology_plan_path).map_err(|error| {
        format!(
            "failed to read Live Corpus Agent Org topology scenario suite {}: {error}",
            topology_plan_path.display()
        )
    })?;
    let topology_plan = toml::from_str::<AgentOrgTopologyScenarioSuite>(&topology_plan_source)
        .map_err(|error| {
            format!("failed to decode Live Corpus Agent Org topology scenario suite: {error}")
        })?;
    let (topology_prompt_contract, topology_scenarios) =
        validate_topology_scenarios(&plan.cases, topology_plan)?;
    let lock_path = qualification_input_path(&plan.lock_path);
    let lock_bytes = std::fs::read(&lock_path).map_err(|error| {
        format!(
            "failed to read Live Corpus lock {}: {error}",
            lock_path.display()
        )
    })?;
    let lock = load_lock(&lock_path)?;
    let resident_sample_count = plan.resident_sample_count;
    let sequential_sample_count = plan.sequential_sample_count;
    let concurrent_sample_count = plan.concurrent_sample_count;
    let cold_load_sample_count = plan
        .cache_states
        .iter()
        .find(|state| state.state == "cold-load")
        .map(|state| state.sample_count)
        .ok_or_else(|| "Live Corpus plan omitted cold-load cache state".to_owned())?;
    let protocol_qualified_case_count = plan.client_protocol.applies_to_case_count;
    let resident_search_latency_budget = ResidentSearchLatencyBudget {
        p50_micros: plan.client_protocol.p50_maximum_micros,
        p99_micros: plan.client_protocol.p99_maximum_micros,
        max_micros: plan.client_protocol.max_maximum_micros,
    };
    let cases = select_qualification_cases(
        plan.cases,
        args.language_id.as_deref(),
        args.resource_id.as_deref(),
    )?;
    let mut prepared_cases = Vec::with_capacity(cases.len());
    for case in cases {
        let topology_scenario = topology_scenarios.get(&case.case_id).ok_or_else(|| {
            format!(
                "Live Corpus topology scenario is unavailable after admission: case={}",
                case.case_id
            )
        })?;
        let agent_prompt = render_agent_prompt(&topology_prompt_contract, topology_scenario);
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
        let repository =
            live_corpus_git_repository_paths(&resource_state_home, &corpus.git.remote)?;
        let checkout_path = repository
            .repository_dir
            .join("checkouts")
            .join(&corpus.git.revision)
            .canonicalize()
            .map_err(|error| {
                format!(
                    "Live Corpus checkout is unavailable: resource={} error={error}",
                    case.resource_id
                )
            })?;
        let current_pointer = artifact_current_pointer(&resource_state_home, &case.resource_id);
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
            || Path::new(&qualification.source_path)
                .canonicalize()
                .map_err(|error| {
                    format!(
                        "Live Corpus qualified source path is unavailable: case={} error={error}",
                        case.case_id
                    )
                })?
                != checkout_path
        {
            return Err(format!(
                "Live Corpus immutable artifact identity drift: case={} artifact={}",
                case.case_id,
                artifact_dir.display()
            ));
        }
        prepared_cases.push(PreparedCase {
            case,
            agent_prompt,
            required_relation_kinds: topology_scenario.required_relation_kinds.clone(),
            composed_search: topology_scenario.composed_search.clone(),
            multi_source_query: topology_scenario.multi_source_query.clone(),
            multi_callable_skeleton_query: topology_scenario.multi_callable_skeleton_query.clone(),
            minimum_composed_candidates: topology_scenario.minimum_composed_candidates,
            expected_coverage_certificate_count: expected_coverage_certificate_count(
                &topology_scenario.route_class,
            )?,
            checkout_path,
            remote: corpus.git.remote.clone(),
            qualification,
            artifact_digest: artifact_digest.to_owned(),
        });
    }
    Ok(PreparedRun {
        args,
        plan_bytes,
        lock_bytes,
        runtime_state_home,
        resource_state_home,
        resident_sample_count,
        sequential_sample_count,
        concurrent_sample_count,
        cold_load_sample_count,
        protocol_qualified_case_count,
        resident_search_latency_budget,
        cases: prepared_cases,
    })
}

fn qualification_input_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

pub(super) fn validate_topology_scenarios(
    search_cases: &[QualificationCase],
    suite: AgentOrgTopologyScenarioSuite,
) -> Result<(String, BTreeMap<String, AgentOrgTopologyScenario>), String> {
    if suite.schema_id != "agent.semantic-protocols.live-corpus-agent-org-topology-scenario-suite"
        || suite.schema_version != "1"
        || suite.prompt_contract.trim().is_empty()
    {
        return Err("unsupported Live Corpus Agent Org topology scenario suite".to_owned());
    }
    let expected = search_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let search_cases_by_id = search_cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut scenarios = BTreeMap::new();
    let mut covered_route_classes = BTreeSet::new();
    for mut scenario in suite.cases {
        let search_case = search_cases_by_id
            .get(scenario.case_id.as_str())
            .ok_or_else(|| {
                format!(
                    "Live Corpus Agent Org topology scenario has no Search/Query case: {}",
                    scenario.case_id
                )
            })?;
        let relation_kinds_are_valid =
            normalize_relation_kinds(&mut scenario.required_relation_kinds);
        if scenario.case_id.is_empty()
            || scenario.resource_id.is_empty()
            || scenario.route_class.is_empty()
            || scenario.reasoning_focus.trim().is_empty()
            || scenario.required_relation_kinds.is_empty()
            || scenario.minimum_composed_candidates < 2
            || scenario.composed_search.trim().is_empty()
            || scenario.multi_source_query.matches("{{selectors}}").count() != 1
            || scenario
                .multi_callable_skeleton_query
                .matches("{{selectors}}")
                .count()
                != 1
            || !relation_kinds_are_valid
        {
            return Err("Live Corpus Agent Org topology scenario is incomplete".to_owned());
        }
        let parsed = agent_semantic_search::parse_progressive_search_playbook_args(&[
            "search".to_owned(),
            "playbook".to_owned(),
            scenario.composed_search.clone(),
        ])
        .map_err(|error| {
            format!(
                "Live Corpus composed Search Scheme is invalid: case={} error={error}",
                scenario.case_id
            )
        })?;
        validate_topology_search_route(&scenario, search_case, &parsed)?;
        covered_route_classes.insert(scenario.route_class.clone());
        validate_workspace_query_set_scheme_template(
            &search_case.language_id,
            &scenario.multi_source_query,
            "source",
        )
        .map_err(|error| {
            format!(
                "Live Corpus composed source Query Scheme is invalid: case={} error={error}",
                scenario.case_id
            )
        })?;
        validate_workspace_query_set_scheme_template(
            &search_case.language_id,
            &scenario.multi_callable_skeleton_query,
            "callable-skeleton",
        )
        .map_err(|error| {
            format!(
                "Live Corpus composed callable-skeleton Query Scheme is invalid: case={} error={error}",
                scenario.case_id
            )
        })?;
        if scenarios
            .insert(scenario.case_id.clone(), scenario)
            .is_some()
        {
            return Err("Live Corpus Agent Org topology scenario is duplicated".to_owned());
        }
    }
    let observed = scenarios.keys().cloned().collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(format!(
            "Live Corpus Agent Org topology scenarios must map one-to-one to Search/Query cases: expected={expected:?} observed={observed:?}"
        ));
    }
    let required_route_classes = BTreeSet::from([
        "regex-truth".to_owned(),
        "ranked-text".to_owned(),
        "structural-syntax".to_owned(),
        "explicit-conjunction".to_owned(),
    ]);
    if covered_route_classes != required_route_classes {
        return Err(format!(
            "Live Corpus Agent Org topology suite must cover every predicate-directed route: observed={covered_route_classes:?} required={required_route_classes:?}"
        ));
    }
    for case in search_cases {
        if scenarios
            .get(&case.case_id)
            .is_none_or(|scenario| scenario.resource_id != case.resource_id)
        {
            return Err(format!(
                "Live Corpus Agent Org topology resource drift: case={}",
                case.case_id
            ));
        }
    }
    Ok((suite.prompt_contract, scenarios))
}

fn normalize_relation_kinds(relation_kinds: &mut Vec<String>) -> bool {
    if relation_kinds.iter().any(String::is_empty) {
        return false;
    }
    relation_kinds.sort();
    relation_kinds.dedup();
    true
}

fn validate_topology_search_route(
    scenario: &AgentOrgTopologyScenario,
    search_case: &QualificationCase,
    parsed: &agent_semantic_search::ProgressiveSearchPlaybookRequest,
) -> Result<(), String> {
    let no_witness = scenario
        .complete_set_witness
        .as_deref()
        .is_none_or(|witness| witness.trim().is_empty());
    let route_matches = match scenario.route_class.as_str() {
        "regex-truth" => {
            no_witness
                && parsed.rg.len() == 1
                && parsed.tantivy.is_empty()
                && parsed.syntax.is_empty()
                && parsed.native_syntax.is_empty()
                && parsed.graph.is_empty()
        }
        "ranked-text" => {
            no_witness
                && parsed.rg.is_empty()
                && parsed.tantivy.len() == 1
                && parsed.syntax.is_empty()
                && parsed.native_syntax.is_empty()
                && parsed.graph.is_empty()
        }
        "structural-syntax" => {
            no_witness
                && parsed.rg.is_empty()
                && parsed.tantivy.is_empty()
                && parsed.syntax.len() == 1
                && parsed.syntax[0].producer == search_case.language_id
                && parsed.native_syntax.is_empty()
                && parsed.graph.is_empty()
        }
        "explicit-conjunction" => {
            !no_witness
                && parsed.rg.len() == 1
                && parsed.tantivy.len() == 1
                && parsed.syntax.is_empty()
                && parsed.native_syntax.is_empty()
                && parsed.graph.is_empty()
                && matches!(
                    parsed.normalized_composition,
                    agent_semantic_search::SearchPlaybookNormalizedComposition::Intersect(_)
                )
        }
        _ => false,
    };
    if !route_matches {
        return Err(format!(
            "Live Corpus topology Search does not match its predicate-directed route or completeness witness: case={} routeClass={}",
            scenario.case_id, scenario.route_class
        ));
    }
    Ok(())
}

pub(super) fn expected_coverage_certificate_count(route_class: &str) -> Result<usize, String> {
    match route_class {
        "regex-truth" | "ranked-text" | "structural-syntax" => Ok(1),
        "explicit-conjunction" => Ok(2),
        _ => Err(format!(
            "Live Corpus topology Search route class is unsupported: {route_class}"
        )),
    }
}

fn render_agent_prompt(prompt_contract: &str, scenario: &AgentOrgTopologyScenario) -> String {
    let witness = scenario
        .complete_set_witness
        .as_deref()
        .filter(|witness| !witness.trim().is_empty())
        .unwrap_or("not-applicable");
    format!(
        "{prompt_contract}\n\nSearch route class: {}\nComplete-set witness: {}\nReasoning focus: {}\nRequired relationship kinds: {}",
        scenario.route_class,
        witness,
        scenario.reasoning_focus,
        scenario.required_relation_kinds.join(", ")
    )
}
