// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn cancellation_probe_has_a_language_neutral_route_operation() {
    assert_eq!(
        crate::ServerClientRoute::CancellationProbe.operation(),
        "lifecycle.cancellation"
    );
    assert_eq!(
        crate::CANCELLATION_PROBE_METHOD,
        "asp.lifecycle.cancellation"
    );
}

#[test]
fn server_catalog_declares_each_shared_method_exactly_once() {
    let digest = format!("blake3-256:{}", "0".repeat(64));
    let catalog = crate::server_method_catalog::server_client_catalog(
        digest.clone(),
        digest,
        vec![crate::ClientTransport::RuntimeIpc],
        Vec::new(),
    )
    .expect("shared Runtime methods form a valid catalog");

    assert_eq!(
        catalog
            .methods
            .iter()
            .filter(|method| method.method == crate::CANCELLATION_PROBE_METHOD)
            .count(),
        1,
        "the shared cancellation route must have one catalog authority"
    );
}

#[test]
fn server_catalog_publishes_the_cancellation_probe() {
    let methods = crate::server_method_catalog::server_client_methods(Vec::new())
        .expect("server method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::server_method_catalog::CANCELLATION_PROBE_METHOD)
        .expect("cancellation probe method");
    assert_eq!(
        method.route_id.as_str(),
        crate::server_method_catalog::CANCELLATION_PROBE_METHOD
    );
    assert!(method.cancellable);
    assert!(!method.streaming);
}

#[test]
fn server_catalog_publishes_the_live_corpus_cache_state_authority() {
    let methods = crate::server_method_catalog::server_client_methods(Vec::new())
        .expect("server method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::LIVE_CORPUS_CACHE_STATE_METHOD)
        .expect("Live Corpus cache-state method");
    assert_eq!(
        method.route_id.as_str(),
        crate::LIVE_CORPUS_CACHE_STATE_METHOD
    );
    assert!(!method.cancellable);
    assert!(!method.streaming);
}

#[test]
fn live_corpus_cache_state_matrix_is_content_bound_and_fail_closed() {
    let warm = crate::LiveCorpusCacheStateRequest {
        schema_id: crate::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "warm-rust-tokio".to_owned(),
        resource_id: "rust.tokio".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        artifact_digest: "a".repeat(64),
        cache_state: "warm-read".to_owned(),
        prepare_action: "reuse-exact-resident-generation".to_owned(),
        mutation_scope: "none".to_owned(),
        expected_generation_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
        expected_root_digest: Some("c".repeat(64)),
    };
    warm.validate().expect("exact warm cache identity");
    let mut stale = warm.clone();
    stale.expected_root_digest = None;
    assert!(stale.validate().is_err());
    let mut released = warm.clone();
    released.cache_state = "released".to_owned();
    released.prepare_action = "release-exact-benchmark-generation".to_owned();
    released.mutation_scope = "benchmark-workspace-generation".to_owned();
    released
        .validate()
        .expect("exact benchmark generation release");
    let mut global = warm;
    global.mutation_scope = "global".to_owned();
    assert!(global.validate().is_err());
}

#[test]
fn live_corpus_cache_receipt_is_bound_to_project_workspace_without_global_authority() {
    let receipt = crate::LiveCorpusCacheStateReceipt {
        schema_id: crate::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "warm-rust-tokio".to_owned(),
        state: "ready".to_owned(),
        cache_state: "warm-read".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace-checkout".to_owned(),
        generation_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
        root_digest: Some("c".repeat(64)),
        resident_generation_evicted: false,
        client_session_evicted: false,
        source_workspace_mutation_count: 0,
        filesystem_delete_count: 0,
        elapsed_micros: 17,
    };
    receipt.validate().expect("project/workspace-bound receipt");
    let value = serde_json::to_value(receipt).expect("serialize receipt");
    assert_eq!(value["projectId"], "repo-project");
    assert_eq!(value["workspaceId"], "workspace-checkout");
    assert!(value.get("workspaceIdentity").is_none());
    assert!(value.get("globalCacheMutationCount").is_none());
}

#[test]
fn northbound_client_requests_are_distinct_from_provider_runtime_requests() {
    let workspace_playbook = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec!["RuntimeAspClient".to_owned()]]),
        tantivy: Some(vec![vec![
            "title:\"Runtime ASP client\"^2 OR body:transport".to_owned(),
        ]]),
        topology: None,
        native_syntax: None,
        graph: None,
        composition: crate::AspClientSearchPlaybookComposition::Intersect {
            children: vec![
                crate::AspClientSearchPlaybookComposition::Leaf {
                    clause: crate::AspClientSearchPlaybookClauseRef {
                        axis: crate::AspClientSearchPlaybookClauseAxis::Rg,
                        block_index: 0,
                    },
                },
                crate::AspClientSearchPlaybookComposition::Leaf {
                    clause: crate::AspClientSearchPlaybookClauseRef {
                        axis: crate::AspClientSearchPlaybookClauseAxis::Tantivy,
                        block_index: 0,
                    },
                },
            ],
        },
        clause_order: vec![
            crate::AspClientSearchPlaybookClauseRef {
                axis: crate::AspClientSearchPlaybookClauseAxis::Rg,
                block_index: 0,
            },
            crate::AspClientSearchPlaybookClauseRef {
                axis: crate::AspClientSearchPlaybookClauseAxis::Tantivy,
                block_index: 0,
            },
        ],
    };
    workspace_playbook
        .validate_schema_identity()
        .expect("workspace playbook identity");

    let query_playbook = crate::AspClientWorkspaceQueryPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: Some("org".to_owned()),
        selectors: vec![
            "org://docs/publication.org#item/heading/Publication".to_owned(),
            "rust://src/lib.rs#item/function/example".to_owned(),
            "rust://crates/runtime/src/lib.rs".to_owned(),
        ],
        projection: "source".to_owned(),
    };
    query_playbook
        .validate_schema_identity()
        .expect("workspace Query Playbook identity");

    let source_index = crate::AspClientSourceIndexLookupRequest {
        schema_id: "agent.semantic-protocols.asp-client-source-index-lookup-request".to_owned(),
        schema_version: "1".to_owned(),
        query: "RuntimeAspClient".to_owned(),
        index_root: "/workspace".to_owned(),
        limit: 8,
    };
    source_index
        .validate_schema_identity()
        .expect("source-index identity");

    let exact = crate::AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: "rust://src/lib.rs#item/function/example".to_owned(),
        projection: "callable-skeleton".to_owned(),
    };
    exact.validate_schema_identity().expect("query identity");

    assert_ne!(
        workspace_playbook.schema_id,
        "agent.semantic-protocols.runtime-provider-search-request"
    );
}

#[test]
fn workspace_playbook_clause_order_is_priority_and_graph_barrier() {
    use crate::{
        AspClientSearchPlaybookClauseAxis as Axis, AspClientSearchPlaybookClauseRef as Clause,
        AspClientSearchPlaybookGraphBlock,
    };

    let request = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec![
            "-e".to_owned(),
            "ClientFrame|Endpoint".to_owned(),
        ]]),
        tantivy: Some(vec![vec![
            "title:\"Client frame\"^2 OR body:Endpoint".to_owned(),
        ]]),
        topology: None,
        native_syntax: None,
        graph: Some(vec![AspClientSearchPlaybookGraphBlock {
            language: "gql".to_owned(),
            argv: vec!["MATCH (a:Owner)-[:CALLS]->(b:Item) RETURN a, b".to_owned()],
        }]),
        composition: crate::AspClientSearchPlaybookComposition::Chain {
            children: vec![
                crate::AspClientSearchPlaybookComposition::Intersect {
                    children: vec![
                        crate::AspClientSearchPlaybookComposition::Leaf {
                            clause: Clause {
                                axis: Axis::Rg,
                                block_index: 0,
                            },
                        },
                        crate::AspClientSearchPlaybookComposition::Leaf {
                            clause: Clause {
                                axis: Axis::Tantivy,
                                block_index: 0,
                            },
                        },
                    ],
                },
                crate::AspClientSearchPlaybookComposition::Leaf {
                    clause: Clause {
                        axis: Axis::Graph,
                        block_index: 0,
                    },
                },
            ],
        },
        clause_order: vec![
            Clause {
                axis: Axis::Rg,
                block_index: 0,
            },
            Clause {
                axis: Axis::Tantivy,
                block_index: 0,
            },
            Clause {
                axis: Axis::Graph,
                block_index: 0,
            },
        ],
    };
    request
        .validate_schema_identity()
        .expect("written acquisition priority followed by Graph is valid");

    let mut acquisition_after_graph = request.clone();
    acquisition_after_graph.clause_order = vec![
        Clause {
            axis: Axis::Graph,
            block_index: 0,
        },
        Clause {
            axis: Axis::Rg,
            block_index: 0,
        },
        Clause {
            axis: Axis::Tantivy,
            block_index: 0,
        },
    ];
    assert!(
        acquisition_after_graph
            .validate_schema_identity()
            .unwrap_err()
            .contains("precede Graph")
    );

    let mut missing_clause = request;
    missing_clause.clause_order.remove(1);
    assert!(
        missing_clause
            .validate_schema_identity()
            .unwrap_err()
            .contains("coverage is invalid")
    );
}

#[test]
fn workspace_search_v1_admits_independent_engine_routes() {
    use crate::{
        AspClientSearchPlaybookClauseAxis as Axis, AspClientSearchPlaybookClauseRef as Clause,
    };

    let base = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec!["RuntimeClient|ResolvedRoute".to_owned()]]),
        tantivy: None,
        topology: None,
        native_syntax: None,
        graph: None,
        composition: crate::AspClientSearchPlaybookComposition::Leaf {
            clause: Clause {
                axis: Axis::Rg,
                block_index: 0,
            },
        },
        clause_order: vec![Clause {
            axis: Axis::Rg,
            block_index: 0,
        }],
    };
    base.validate_schema_identity()
        .expect("regex does not require a Tantivy partner");

    let duplicate = crate::AspClientWorkspaceSearchPlaybookRequest {
        rg: Some(vec![
            vec!["RuntimeClient|ResolvedRoute".to_owned()],
            vec!["RuntimeClient|ResolvedRoute".to_owned()],
        ]),
        clause_order: vec![
            Clause {
                axis: Axis::Rg,
                block_index: 0,
            },
            Clause {
                axis: Axis::Rg,
                block_index: 1,
            },
        ],
        composition: crate::AspClientSearchPlaybookComposition::Intersect {
            children: vec![
                crate::AspClientSearchPlaybookComposition::Leaf {
                    clause: Clause {
                        axis: Axis::Rg,
                        block_index: 0,
                    },
                },
                crate::AspClientSearchPlaybookComposition::Leaf {
                    clause: Clause {
                        axis: Axis::Rg,
                        block_index: 1,
                    },
                },
            ],
        },
        ..base.clone()
    };
    assert!(
        duplicate
            .validate_schema_identity()
            .unwrap_err()
            .contains("redundant work")
    );

    let structural = crate::AspClientWorkspaceSearchPlaybookRequest {
        rg: None,
        native_syntax: Some(vec!["rust://src/lib.rs#item/function/example".to_owned()]),
        clause_order: vec![Clause {
            axis: Axis::NativeSyntax,
            block_index: 0,
        }],
        composition: crate::AspClientSearchPlaybookComposition::Leaf {
            clause: Clause {
                axis: Axis::NativeSyntax,
                block_index: 0,
            },
        },
        ..base
    };
    structural
        .validate_schema_identity()
        .expect("structural Search does not require lexical acquisition");
}

#[test]
fn workspace_search_v1_rejects_flattened_or_domain_invalid_composition() {
    use crate::{
        AspClientSearchPlaybookClauseAxis as Axis, AspClientSearchPlaybookClauseRef as Clause,
        AspClientSearchPlaybookComposition as Composition,
    };

    let mut request = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec!["RuntimeClient|ResolvedRoute".to_owned()]]),
        tantivy: None,
        topology: None,
        native_syntax: None,
        graph: None,
        composition: Composition::Leaf {
            clause: Clause {
                axis: Axis::Rg,
                block_index: 0,
            },
        },
        clause_order: vec![Clause {
            axis: Axis::Rg,
            block_index: 0,
        }],
    };

    request.composition = Composition::Leaf {
        clause: Clause {
            axis: Axis::Rg,
            block_index: 1,
        },
    };
    assert!(
        request
            .validate_schema_identity()
            .unwrap_err()
            .contains("exactly match clauseOrder")
    );

    request.native_syntax = Some(vec!["rust://src/lib.rs#item/function/example".to_owned()]);
    request.clause_order = vec![
        Clause {
            axis: Axis::Rg,
            block_index: 0,
        },
        Clause {
            axis: Axis::NativeSyntax,
            block_index: 0,
        },
    ];
    request.composition = Composition::Intersect {
        children: vec![
            Composition::Leaf {
                clause: request.clause_order[0].clone(),
            },
            Composition::Leaf {
                clause: request.clause_order[1].clone(),
            },
        ],
    };
    assert!(
        request
            .validate_schema_identity()
            .unwrap_err()
            .contains("rg/Tantivy/Topology set branches")
    );
}
