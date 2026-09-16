// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use super::qualification_receipt_path;
use crate::command::live_corpus::qualification::client_protocol::render_workspace_query_scheme_source;
use crate::command::live_corpus::qualification::client_protocol::workspace_query_qualification_request;
use crate::command::live_corpus::qualification::contract::AgentOrgTopologyEvidence;
use crate::command::live_corpus::qualification::contract::AgentOrgTopologyScenarioSuite;
use crate::command::live_corpus::qualification::contract::LatencyDistribution;
use crate::command::live_corpus::qualification::contract::QualificationCase;
use crate::command::live_corpus::qualification::contract::QualificationPlan;
use crate::command::live_corpus::qualification::protocol_model::PublicRouteTerminal;
use crate::command::live_corpus::qualification::protocol_model::typed_terminal;
use crate::command::live_corpus::qualification::query_protocol::render_workspace_query_set_scheme_source;
use crate::command::live_corpus::qualification::query_protocol::validate_workspace_query_set_scheme_template;
use crate::command::live_corpus::qualification::runner_contract::parse_args;
use crate::command::live_corpus::qualification::runner_contract::select_qualification_cases;
use crate::command::live_corpus::qualification::runner_contract::validate_plan;
use crate::command::live_corpus::qualification::runner_prepare::artifact_current_pointer;
use crate::command::live_corpus::qualification::runner_prepare::validate_topology_scenarios;
use crate::command::live_corpus::qualification::workspace_fixture::IsolatedBenchmarkWorkspace;

fn case(case_id: &str, language_id: &str, resource_id: &str) -> QualificationCase {
    QualificationCase {
        case_id: case_id.to_owned(),
        resource_id: resource_id.to_owned(),
        scenario_id: format!("{resource_id}.scenario"),
        language_id: language_id.to_owned(),
        provider_id: format!("asp-{language_id}"),
        scenario_classes: vec![
            "explicit-conjunction".to_owned(),
            "exact-parser-owner".to_owned(),
            "zero-match".to_owned(),
        ],
        search: format!(
            "(search (producers (language {language_id})) (intersect (rg \"-n\" \"owner\") (tantivy \"title:owner OR body:owner\")))"
        ),
        zero_match_search: format!(
            "(search (producers (language {language_id})) (rg \"-n\" \"definitely-absent\"))"
        ),
        source_query: format!(
            "(query (producers (language {language_id})) (select (selectors {{{{selector}}}}) (projection source) (output json)))"
        ),
        callable_skeleton_query: format!(
            "(query (producers (language {language_id})) (select (selectors {{{{selector}}}}) (projection callable-skeleton) (output json)))"
        ),
        minimum_candidates: 1,
        maximum_search_micros: 500_000,
        maximum_resident_query_micros: 1_000,
        required_telemetry_events: Vec::new(),
    }
}

#[test]
fn qualification_search_is_lowered_through_one_scheme_expression() {
    let source = "(search (producers (language rust)) (intersect (rg \"-n\" \"-F\" \"owner \\\"identity\\\"\") (tantivy \"title:\\\"owner identity\\\"^2 OR body:owner\")))";
    assert!(source.starts_with("(search (producers (language rust)) (intersect (rg "));
    assert!(source.contains("owner \\\"identity\\\""));

    let request = agent_semantic_search::parse_progressive_search_playbook_args(&[
        "search".to_owned(),
        "playbook".to_owned(),
        source.to_owned(),
    ])
    .expect("Scheme-lowered Search request");
    assert_eq!(request.language.as_deref(), Some("rust"));
    assert_eq!(request.documents, None);
    assert!(!request.rg.is_empty());
    assert!(!request.tantivy.is_empty());
    assert_eq!(request.clause_order.len(), 2);
}

#[test]
fn qualification_query_is_lowered_through_one_scheme_expression() {
    let selector = "rust://src/lib.rs#item/function/run";
    let template = "(query (producers (language rust)) (select (selectors {{selector}}) (projection callable-skeleton) (output json)))";
    let source = render_workspace_query_scheme_source(template, selector)
        .expect("canonical Query Scheme source");
    assert_eq!(
        source,
        "(query (producers (language rust)) (select (selectors \"rust://src/lib.rs#item/function/run\") (projection callable-skeleton) (output json)))"
    );
    let request =
        workspace_query_qualification_request("rust", selector, "callable-skeleton", template)
            .expect("Scheme-lowered Query request");
    assert_eq!(request.language.as_deref(), Some("rust"));
    assert_eq!(request.documents, None);
    assert_eq!(request.selectors, [selector]);
    assert_eq!(request.projection, "callable-skeleton");
}

#[test]
fn composed_query_preserves_the_ordered_selector_set() {
    let template = "(query (producers (language rust)) (select (selectors {{selectors}}) (projection source) (output json)))";
    validate_workspace_query_set_scheme_template("rust", template, "source")
        .expect("valid multi-selector Query Scheme");
    let source = render_workspace_query_set_scheme_source(
        template,
        &[
            "rust://src/a.rs#item/a".to_owned(),
            "rust://src/b.rs#item/b".to_owned(),
        ],
    )
    .expect("rendered selector set");
    assert!(source.contains("\"rust://src/a.rs#item/a\" \"rust://src/b.rs#item/b\""));
}

#[test]
fn live_corpus_plan_exercises_structural_search_for_every_registered_language() {
    let plan: QualificationPlan = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("baseline Scheme suite");
    validate_plan(&plan).expect("predicate-directed V1 qualification plan");
}

#[test]
fn live_corpus_plan_rejects_a_language_without_structural_search() {
    let mut plan: QualificationPlan = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("baseline Scheme suite");
    for case in plan
        .cases
        .iter_mut()
        .filter(|case| case.language_id == "org")
    {
        case.scenario_classes[0] = "regex-truth".to_owned();
        case.search = "(search (producers (documents org)) (rg \"-n\" \"headline\"))".to_owned();
    }
    let error = validate_plan(&plan).expect_err("missing structural language coverage");
    assert!(
        error.contains("structural Search for every registered language"),
        "error={error}"
    );
}

#[test]
fn every_live_corpus_case_admits_its_complex_scheme_intent() {
    let plan: QualificationPlan = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("baseline Scheme suite");
    let suite: AgentOrgTopologyScenarioSuite = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml"
    ))
    .expect("complex Scheme suite");
    let (_, admitted) =
        validate_topology_scenarios(&plan.cases, suite).expect("all complex Scheme intents admit");
    assert_eq!(admitted.len(), 17);
}

#[test]
fn topology_intersection_cannot_reuse_relation_coverage_as_set_completeness() {
    let plan: QualificationPlan = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("baseline Scheme suite");
    let mut suite: AgentOrgTopologyScenarioSuite = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml"
    ))
    .expect("topology Scheme suite");
    let scenario = suite
        .cases
        .iter_mut()
        .find(|scenario| scenario.route_class == "ranked-text")
        .expect("ranked scenario");
    let language = plan
        .cases
        .iter()
        .find(|case| case.case_id == scenario.case_id)
        .expect("matching baseline case")
        .language_id
        .clone();
    scenario.route_class = "explicit-conjunction".to_owned();
    scenario.composed_search = format!(
        "(search (producers (language {language})) (intersect (rg \"-n\" \"owner\") (tantivy \"title:\\\"owner identity\\\"^2 OR body:owner\")))"
    );

    let error = validate_topology_scenarios(&plan.cases, suite)
        .expect_err("an intersection without acquisition-set receipts must fail closed");
    assert!(error.contains("predicate-directed route"), "error={error}");
}

#[test]
fn topology_route_class_must_match_the_scheme_ast() {
    let plan: QualificationPlan = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("baseline Scheme suite");
    let mut suite: AgentOrgTopologyScenarioSuite = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml"
    ))
    .expect("topology Scheme suite");
    let scenario = suite
        .cases
        .iter_mut()
        .find(|scenario| scenario.route_class == "ranked-text")
        .expect("ranked-text scenario");
    scenario.composed_search = format!(
        "(search (producers (language {})) (rg \"-n\" \"owner\"))",
        plan.cases
            .iter()
            .find(|case| case.case_id == scenario.case_id)
            .expect("matching baseline case")
            .language_id
    );

    let error =
        validate_topology_scenarios(&plan.cases, suite).expect_err("route drift must fail closed");
    assert!(error.contains("predicate-directed route"), "error={error}");
}

#[test]
fn topology_suite_does_not_claim_acquisition_completeness_from_frontier_coverage() {
    let suite: AgentOrgTopologyScenarioSuite = toml::from_str(include_str!(
        "../../../../../benchmarks/live-corpus-agent-org-topology-scenarios.v1.toml"
    ))
    .expect("topology Scheme suite");
    assert!(
        suite
            .cases
            .iter()
            .all(|scenario| scenario.route_class != "explicit-conjunction"
                && !scenario.composed_search.contains("(intersect "))
    );
}

fn error_frame(
    reason_kind: &str,
    admission_state: &str,
) -> agent_semantic_client_protocol::ClientFrame {
    serde_json::from_value(serde_json::json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client-frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "live-corpus-session",
        "projectId": "repo-live-corpus",
        "workspaceId": "workspace-live-corpus",
        "requestId": "live-corpus-request",
        "outcome": "error",
        "error": {
            "reasonKind": reason_kind,
            "message": "generation is not ready",
            "details": {
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                "schemaVersion": "1",
                "state": "failed",
                "operationId": "live-corpus-request",
                "projectId": "repo-live-corpus",
                "workspaceId": "workspace-live-corpus",
                "languageId": "rust",
                "providerId": "asp-rust",
                "requestedSelector": "rust:item:test",
                "resolvedSelector": null,
                "projectionKind": "source",
                "phase": "workspace-generation-admission",
                "reasonKind": reason_kind,
                "generationDigest": null,
                "rootDigest": null,
                "residentReadElapsedMicros": 0,
                "serviceElapsedMicros": 1,
                "elapsedMicros": 1,
                "workCounters": {
                    "databaseReadCount": 0,
                    "filesystemReadCount": 0,
                    "providerProcessCount": 0,
                    "schedulerTaskCount": 0,
                    "socketOperationCount": 0
                },
                "details": {"admissionState": admission_state}
            }
        }
    }))
    .expect("typed ASP Client error frame fixture")
}

#[test]
fn persistent_queued_is_a_typed_rejecting_terminal() {
    let terminal = typed_terminal(error_frame("runtime-generation-queued", "Queued"))
        .expect("typed Queued terminal");
    assert!(matches!(terminal, PublicRouteTerminal::Queued(_)));
}

#[test]
fn persistent_building_is_a_typed_rejecting_terminal() {
    let terminal = typed_terminal(error_frame("runtime-generation-building", "Building"))
        .expect("typed Building terminal");
    assert!(matches!(terminal, PublicRouteTerminal::Building(_)));
}

#[test]
fn qualification_accepts_one_explicit_resource_selector() {
    let args = parse_args(&["--resource".to_owned(), "rust.bytes".to_owned()])
        .expect("parse one resource selector");
    assert_eq!(args.resource_id.as_deref(), Some("rust.bytes"));
}

#[test]
fn qualification_rejects_a_missing_resource_selector_value() {
    let error =
        parse_args(&["--resource".to_owned()]).expect_err("resource selector value is required");
    assert!(error.contains("requires a resource id after --resource"));
}

#[test]
fn qualification_rejects_duplicate_resource_selectors() {
    let error = parse_args(&[
        "--resource".to_owned(),
        "rust.bytes".to_owned(),
        "--resource".to_owned(),
        "rust.tokio".to_owned(),
    ])
    .expect_err("one atomic resource selector is allowed");
    assert!(error.contains("accepts exactly one --resource option"));
}

#[test]
fn qualification_requires_one_atomic_resource_selector() {
    let error = parse_args(&[]).expect_err("an omitted resource must not fan out");
    assert_eq!(error, "live_corpus qualify requires exactly one --resource");
}

#[test]
fn qualification_rejects_language_fan_out() {
    let error = parse_args(&[
        "--language".to_owned(),
        "rust".to_owned(),
        "--resource".to_owned(),
        "rust.tokio".to_owned(),
    ])
    .expect_err("language selection must not create a multi-workspace test process");
    assert_eq!(error, "unknown live_corpus qualify option: --language");
}

#[test]
fn language_selection_returns_every_case_for_that_language_only() {
    let selected = select_qualification_cases(
        vec![
            case("rust-bytes", "rust", "rust.bytes"),
            case("python-django", "python", "python.django"),
            case("rust-tokio", "rust", "rust.tokio"),
        ],
        Some("rust"),
        None,
    )
    .expect("select all Rust corpora");
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().all(|case| case.language_id == "rust"));
}

#[test]
fn language_and_resource_selection_fail_closed_on_identity_mismatch() {
    let error = select_qualification_cases(
        vec![case("python-django", "python", "python.django")],
        Some("rust"),
        Some("python.django"),
    )
    .expect_err("cross-language resource selection must fail closed");
    assert!(error.contains("selection was not found"));
}

#[test]
fn qualification_receipts_are_partitioned_by_language_and_resource() {
    let state_home = Path::new("/state-home");
    assert_eq!(
        qualification_receipt_path(state_home, None, None),
        state_home.join("resources/live-corpus/receipts/search-query-qualification.json")
    );
    assert_eq!(
        qualification_receipt_path(state_home, Some("rust"), None),
        state_home.join(
            "resources/live-corpus/receipts/search-query-qualification/by-language/rust.json"
        )
    );
    assert_eq!(
        qualification_receipt_path(state_home, Some("rust"), Some("rust.tokio")),
        state_home.join(
            "resources/live-corpus/receipts/search-query-qualification/by-resource/rust.tokio.json"
        )
    );
}

#[test]
fn immutable_artifact_pointer_uses_the_state_home_resource_authority() {
    assert_eq!(
        artifact_current_pointer(Path::new("/state-home"), "rust.bytes"),
        Path::new("/state-home")
            .join("resources/live-corpus/artifacts/by-resource/rust.bytes/current")
    );
}

#[test]
fn latency_distribution_reports_executed_sample_count_and_nearest_ranks() {
    let distribution = LatencyDistribution::from_samples((1..=100).rev().collect())
        .expect("non-empty latency distribution");
    assert_eq!(distribution.sample_count, 100);
    assert_eq!(distribution.min_micros, 1);
    assert_eq!(distribution.p50_micros, 50);
    assert_eq!(distribution.p95_micros, 95);
    assert_eq!(distribution.p99_micros, 99);
    assert_eq!(distribution.max_micros, 100);
}

#[test]
fn topology_evidence_binds_exact_query_bytes_and_base_topology() {
    let digest = |byte: char| format!("blake3-256:{}", byte.to_string().repeat(64));
    let admit = |source: &[u8]| {
        let selectors = vec![
            "rust://src/lib.rs#item/function/run".to_owned(),
            "rust://src/runtime.rs#item/function/dispatch".to_owned(),
        ];
        AgentOrgTopologyEvidence::admit(
            "rust.bytes.core".to_owned(),
            "rust.bytes".to_owned(),
            digest('1'),
            digest('2'),
            selectors[0].clone(),
            selectors.clone(),
            "search-composed-1".to_owned(),
            "(search (producers (language rust)) (intersect (rg \"run\") (tantivy \"run\")))",
            "query-source-1".to_owned(),
            vec![
                (selectors[0].clone(), source.to_vec()),
                (selectors[1].clone(), b"pub fn dispatch() {}\n".to_vec()),
            ],
            "query-skeleton-1".to_owned(),
            selectors
                .into_iter()
                .map(|selector| {
                    (
                        selector,
                        br#"{"schemaId":"agent.semantic-protocols.callable-skeleton"}"#.to_vec(),
                    )
                })
                .collect(),
            "Analyze exact evidence and return an Org topology contribution.".to_owned(),
            vec!["dispatches-to".to_owned()],
        )
        .expect("admitted exact Query evidence")
    };
    let baseline = admit(b"pub fn run() {}\n");
    let changed = admit(b"pub fn run() { todo!() }\n");
    assert_ne!(baseline.evidence_digest, changed.evidence_digest);
    assert_eq!(baseline.base_topology_generation_digest, digest('2'));
    assert_eq!(baseline.terminal.state, "ready");
    assert_eq!(baseline.terminal.terminal_count, 1);
    assert!(baseline.terminal.reason_kind.is_none());

    let mut self_attested = baseline;
    self_attested.source_materializations[0].bytes_base64 = "bXV0YXRlZA==".to_owned();
    assert!(
        self_attested
            .validate()
            .expect_err("mutated evidence must fail closed")
            .contains("derived-identity-mismatch")
    );
}

#[test]
fn isolated_workspace_preserves_bytes_and_has_a_distinct_runtime_identity() {
    let temp = tempfile::tempdir().expect("temporary Live Corpus root");
    let source = temp.path().join("immutable-source");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::write(source.join("lib.rs"), "pub fn live_corpus() {}\n").expect("source file");
    let source_identity = agent_semantic_client_db::AgentSessionRegistry::workspace_id(&source)
        .expect("source workspace identity");

    let isolated = IsolatedBenchmarkWorkspace::materialize(
        temp.path(),
        &source,
        "rust.real-library",
        &"a".repeat(64),
        "https://github.com/tokio-rs/bytes.git",
    )
    .expect("isolated benchmark workspace");
    assert_ne!(isolated.workspace_identity, source_identity);
    assert_eq!(
        std::fs::read(isolated.path.join("lib.rs")).expect("isolated bytes"),
        b"pub fn live_corpus() {}\n"
    );
    assert!(
        isolated
            .path
            .join(agent_semantic_topology::PROJECT_TOPOLOGY_MANIFEST_PATH)
            .is_file()
    );
    assert!(!source.join(".agents/asp/topology/manifest.org").exists());
    let isolated_path = isolated.path.clone();
    isolated.cleanup().expect("scoped workspace cleanup");
    assert!(!isolated_path.exists());
    assert!(source.join("lib.rs").exists());
}
