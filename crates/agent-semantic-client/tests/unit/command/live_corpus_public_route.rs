// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn live_corpus_test_is_separate_and_forwards_only_typed_public_client_requests() {
    let manifest = include_str!("../../../Cargo.toml");
    assert!(manifest.contains("name = \"live_corpus\""));
    assert!(manifest.contains("required-features = [\"live-corpus-test\"]"));
    assert!(manifest.contains("required-features = [\"asp-bin\"]"));
    assert!(manifest.contains("live-corpus-test = []"));
    assert!(!manifest.contains("live-corpus-test = [\"asp-bin\"]"));
    assert!(!manifest.contains("name = \"asp-live-corpus"));

    let library = include_str!("../../../src/lib.rs");
    assert!(library.contains("pub mod live_corpus_test;"));
    assert!(!library.contains("pub use live_corpus_test::run_live_corpus_test"));

    let runner = include_str!("../../integration/live_corpus/runner.rs");
    assert!(runner.contains("ASP_LIVE_CORPUS_SERVER_ARTIFACT"));
    assert!(runner.contains("ASP_LIVE_CORPUS_PROVIDER_WORKSPACE_DESCRIPTOR"));
    assert!(runner.contains("materialize_provider_workspace_artifact"));
    assert!(!runner.contains("command::install_provider"));
    assert!(runner.contains("provider-artifact-digest="));
    assert!(!runner.contains("symlink("));
    for forbidden in [
        "CARGO_BIN_EXE_asp",
        "RuntimeArtifactStateLayout",
        "verify_runtime_artifact_bound_bundle",
        concat!("mod ", "harness"),
    ] {
        assert!(
            !runner.contains(forbidden),
            "Live Corpus runner retained forbidden product or test-namespace coupling: {forbidden}"
        );
    }

    let justfile = include_str!("../../../../../Justfile");
    assert!(
        justfile.contains("--no-default-features --features live-corpus-test --test live_corpus")
    );

    let product_dispatch = include_str!("../../../src/command/dispatch.rs");
    let product_help = include_str!("../../../src/command/cli_help_model.rs");
    assert!(!product_dispatch.contains("Some(\"live-corpus\")"));
    assert!(!product_help.contains("\"live-corpus\""));

    let owner = concat!(
        include_str!("../../../src/command/live_corpus.rs"),
        include_str!("../../../src/command/live_corpus_qualification/client_protocol.rs"),
        include_str!("../../../src/command/live_corpus_qualification/query_protocol.rs"),
        include_str!("../../../src/command/live_corpus_qualification/runner.rs"),
    );
    for required in [
        "ensure_healthy_runtime_server_for_bounded_operation",
        "RuntimeLanguageCommandClient",
        "LanguageCommandClient",
        "LanguageCommandRequest",
        "parse_progressive_search_playbook_args",
        "parse_progressive_query_args",
        "live-corpus-scheme-scenarios.v1.toml",
        "live-corpus-agent-org-topology-scenarios.v1.toml",
        "render_workspace_query_scheme_source",
        "search_receipt_for_scheme",
        "public_query_set",
        "validate_workspace_query_set_scheme_template",
        "WorkspaceQueryPlaybook",
    ] {
        assert!(
            owner.contains(required),
            "Live Corpus test is missing typed forwarding evidence: {required}"
        );
    }
    for forbidden in [
        "WorkspaceDbIpcSession",
        ".provider_search(",
        "read_runtime_exact_projection",
        "restore_runtime_generation_from_pointer",
        concat!("AspClientProtocol", "HttpClient"),
        "rg: Some(vec![search.rg.clone()])",
        "tantivy: Some(vec![search.tantivy.clone()])",
        "LanguageCommandOperation::ExactQuery",
        "AspClientExactQueryRequest",
    ] {
        assert!(
            !owner.contains(forbidden),
            "Live Corpus test bypassed the typed public client through {forbidden}"
        );
    }

    let transport_owner =
        include_str!("../../../../agent-semantic-client/src/runtime_language_client.rs");
    assert!(transport_owner.contains("AspClientGrpcTransport"));
    assert!(!transport_owner.contains(concat!("AspClientProtocol", "HttpClient")));

    let qualification_runner =
        include_str!("../../../src/command/live_corpus_qualification/runner.rs");
    for required in [
        "tokio::task::JoinSet::new()",
        "case_tasks.spawn(async move",
        "join_tasks_in_plan_order(",
        "for (_, (qualified, evidence, cancellation_elapsed)) in completed_cases",
        "workspace_scheduling: \"tokio-join-set\"",
    ] {
        assert!(
            qualification_runner.contains(required),
            "Live Corpus multi-workspace Tokio gate is missing: {required}"
        );
    }
    assert!(
        !qualification_runner.contains("Semaphore"),
        "Live Corpus runner must not add a leaf workspace concurrency limit"
    );
}
