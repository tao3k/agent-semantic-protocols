// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use super::DEFAULT_PLAN_PATH;
use super::QualificationCase;
use super::QualificationPlan;
use super::QualifyArgs;

pub(super) fn validate_plan(plan: &QualificationPlan) -> Result<(), String> {
    if plan.schema_id != "agent.semantic-protocols.live-corpus-search-query-qualification-plan"
        || plan.schema_version != "1"
    {
        return Err("unsupported Live Corpus qualification plan schema".to_owned());
    }
    if plan.cases.is_empty() || plan.required_languages.is_empty() {
        return Err("Live Corpus qualification plan is empty".to_owned());
    }
    if plan.resident_sample_count < 128 {
        return Err(format!(
            "Live Corpus resident sample count is below the shared minimum: observed={} minimum=128",
            plan.resident_sample_count
        ));
    }
    if plan.sequential_sample_count != 10 || plan.concurrent_sample_count != 32 {
        return Err(format!(
            "Live Corpus execution matrix does not match the shared contract: sequential={} concurrent={}",
            plan.sequential_sample_count, plan.concurrent_sample_count
        ));
    }
    let required_failure_injections = std::collections::BTreeSet::from([
        "cancel-before-terminal",
        "bounded-mailbox-saturation",
        "stale-content-binding",
    ]);
    let observed_failure_injections = plan
        .failure_injections
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if observed_failure_injections != required_failure_injections {
        return Err(format!(
            "Live Corpus failure-injection matrix does not match the shared contract: observed={observed_failure_injections:?} required={required_failure_injections:?}"
        ));
    }
    let observed_cache_states = plan
        .cache_states
        .iter()
        .map(|state| {
            (
                state.state.as_str(),
                state.prepare_action.as_str(),
                state.mutation_scope.as_str(),
                state.sample_count,
            )
        })
        .collect::<Vec<_>>();
    let required_cache_states = vec![
        (
            "cold-build",
            "new-isolated-workspace-generation",
            "benchmark-workspace-generation",
            1,
        ),
        (
            "cold-load",
            "evict-resident-generation-only",
            "benchmark-workspace-generation",
            10,
        ),
        ("warm-read", "reuse-exact-resident-generation", "none", 128),
        (
            "released",
            "release-exact-benchmark-generation",
            "benchmark-workspace-generation",
            1,
        ),
    ];
    if observed_cache_states != required_cache_states {
        return Err(format!(
            "Live Corpus cache-state matrix does not match the shared contract: observed={observed_cache_states:?} required={required_cache_states:?}"
        ));
    }
    let client_protocol = &plan.client_protocol;
    if client_protocol.protocol_id != "agent.semantic-protocols.client"
        || client_protocol.protocol_version != "1"
        || client_protocol.transport != "grpc-tokio-streams"
        || client_protocol.workspace_scheduling != "tokio-join-set"
        || client_protocol.phases
            != [
                "initialize",
                "catalog",
                "request",
                "cancel",
                "cancelled",
                "shutdown",
            ]
        || client_protocol.required_telemetry_events
            != [
                "client_protocol_initialize",
                "client_protocol_catalog",
                "client_protocol_request",
                "client_protocol_cancel",
                "client_protocol_cancelled",
                "client_protocol_shutdown",
            ]
        || client_protocol.applies_to_case_count != 17
        || client_protocol.maximum_resident_micros != 1000
        || client_protocol.session_policy != "one-initialize-per-session"
        || client_protocol.ready_effects != ["mpsc", "oneshot", "cancel", "response"]
        || client_protocol.forbidden_ready_effects
            != [
                "process",
                "filesystem",
                "dbWrite",
                "generationMutation",
                "providerActivation",
                "controlPoll",
            ]
        || client_protocol.non_ready_dispatch_count != 0
        || client_protocol.residual_task_count != 0
        || client_protocol.p50_maximum_micros != 250
        || client_protocol.p99_maximum_micros != 700
        || client_protocol.max_maximum_micros != 1000
    {
        return Err("Live Corpus client protocol contract does not match shared schema".to_owned());
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
            || case.query.owner_view != "items"
            || case.query.projection_scope != "live-corpus"
            || case.search.rg.is_empty()
            || case.search.tantivy.is_empty()
            || case.zero_match_search.rg.is_empty()
            || case.zero_match_search.tantivy.is_empty()
        {
            return Err(format!(
                "Live Corpus case is not an ordinary public search/query route: case={}",
                case.case_id
            ));
        }
    }
    Ok(())
}

pub(super) fn parse_args(args: &[String]) -> Result<QualifyArgs, String> {
    let mut plan_path = PathBuf::from(DEFAULT_PLAN_PATH);
    let mut resource_id = None;
    let mut language_id = None;
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
            "--language" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    "live-corpus qualify requires a language id after --language".to_owned()
                })?;
                if value.trim().is_empty() {
                    return Err("live-corpus qualify language id must be non-empty text".to_owned());
                }
                if language_id.replace(value.clone()).is_some() {
                    return Err(
                        "live-corpus qualify accepts exactly one --language option".to_owned()
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
        language_id,
        json,
    })
}

pub(super) fn select_qualification_cases(
    cases: Vec<QualificationCase>,
    language_id: Option<&str>,
    resource_id: Option<&str>,
) -> Result<Vec<QualificationCase>, String> {
    if language_id.is_none() && resource_id.is_none() {
        return Ok(cases);
    }
    let selected = cases
        .into_iter()
        .filter(|case| {
            language_id.is_none_or(|language_id| case.language_id == language_id)
                && resource_id.is_none_or(|resource_id| case.resource_id == resource_id)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!(
            "Live Corpus qualification selection was not found in plan: language={} resource={}",
            language_id.unwrap_or("*"),
            resource_id.unwrap_or("*")
        ));
    }
    if let Some(resource_id) = resource_id
        && selected.len() != 1
    {
        return Err(format!(
            "Live Corpus qualification resource is duplicated in plan: resource={} count={}",
            resource_id,
            selected.len()
        ));
    }
    Ok(selected)
}
