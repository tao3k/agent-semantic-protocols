// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use super::IsolatedBenchmarkWorkspace;
use super::parse_args;
use super::qualification_receipt_path;
use super::select_qualification_cases;
use crate::command::live_corpus::qualification::client_protocol::PublicRouteTerminal;
use crate::command::live_corpus::qualification::client_protocol::typed_terminal;
use crate::command::live_corpus::qualification::contract::LatencyDistribution;
use crate::command::live_corpus::qualification::contract::QualificationCase;
use crate::command::live_corpus::qualification::contract::QualificationQuery;
use crate::command::live_corpus::qualification::contract::QualificationSearch;

fn case(case_id: &str, language_id: &str, resource_id: &str) -> QualificationCase {
    QualificationCase {
        case_id: case_id.to_owned(),
        resource_id: resource_id.to_owned(),
        scenario_id: format!("{resource_id}.scenario"),
        language_id: language_id.to_owned(),
        provider_id: format!("asp-{language_id}"),
        search: QualificationSearch {
            method: "lexical".to_owned(),
            terms: vec!["owner".to_owned()],
            view: "seeds".to_owned(),
            minimum_candidates: 1,
            maximum_resident_micros: 1_000,
        },
        query: QualificationQuery {
            selector_strategy: "first-ranked-parser-owned".to_owned(),
            owner_view: "items".to_owned(),
            projection_scope: "live-corpus".to_owned(),
            maximum_resident_micros: 1_000,
        },
        zero_match_terms: vec!["definitely-absent".to_owned()],
        required_telemetry_events: Vec::new(),
    }
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
fn qualification_accepts_one_explicit_language_selector() {
    let args = parse_args(&["--language".to_owned(), "rust".to_owned()])
        .expect("parse one language selector");
    assert_eq!(args.language_id.as_deref(), Some("rust"));
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
        state_home.join("runtime/live-corpus/search-query-qualification.json")
    );
    assert_eq!(
        qualification_receipt_path(state_home, Some("rust"), None),
        state_home.join("runtime/live-corpus/search-query-qualification/by-language/rust.json")
    );
    assert_eq!(
        qualification_receipt_path(state_home, Some("rust"), Some("rust.tokio")),
        state_home
            .join("runtime/live-corpus/search-query-qualification/by-resource/rust.tokio.json")
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
    )
    .expect("isolated benchmark workspace");
    assert_ne!(isolated.workspace_identity, source_identity);
    assert_eq!(
        std::fs::read(isolated.path.join("lib.rs")).expect("isolated bytes"),
        b"pub fn live_corpus() {}\n"
    );
    let isolated_path = isolated.path.clone();
    isolated.cleanup().expect("scoped workspace cleanup");
    assert!(!isolated_path.exists());
    assert!(source.join("lib.rs").exists());
}
