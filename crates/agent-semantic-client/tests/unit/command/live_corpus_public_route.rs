// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn live_corpus_test_is_separate_and_forwards_only_typed_public_client_requests() {
    let manifest = include_str!("../../../Cargo.toml");
    assert!(manifest.contains("name = \"live_corpus\""));
    assert!(manifest.contains("required-features = [\"live-corpus-test\"]"));
    assert!(!manifest.contains("name = \"asp-live-corpus"));

    let product_dispatch = include_str!("../../../src/command/dispatch.rs");
    let product_help = include_str!("../../../src/command/cli_help_model.rs");
    assert!(!product_dispatch.contains("Some(\"live-corpus\")"));
    assert!(!product_help.contains("\"live-corpus\""));

    let owner = concat!(
        include_str!("../../../src/command/live_corpus.rs"),
        include_str!("../../../src/command/live_corpus_qualification/client_protocol.rs"),
        include_str!("../../../src/command/live_corpus_qualification/runner.rs"),
    );
    for required in [
        "ensure_healthy_runtime_server_for_bounded_operation",
        "RuntimeLanguageCommandClient",
        "LanguageCommandClient",
        "LanguageCommandRequest",
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
        "for (_, (qualified, cancellation_elapsed)) in completed_cases",
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
