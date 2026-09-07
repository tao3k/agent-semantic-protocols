// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn cli_live_corpus_forwards_only_typed_public_client_requests() {
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
            "Live Corpus public route is missing typed forwarding evidence: {required}"
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
            "Live Corpus CLI bypassed the typed public client through {forbidden}"
        );
    }

    let transport_owner =
        include_str!("../../../../agent-semantic-client/src/runtime_language_client.rs");
    assert!(transport_owner.contains("AspClientGrpcTransport"));
    assert!(!transport_owner.contains(concat!("AspClientProtocol", "HttpClient")));
}
