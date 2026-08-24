use agent_semantic_search_projection::{
    ProviderGraphEvidence, SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID, SemanticMutationClass,
    SemanticSearchAlgorithmEvidence, SemanticSearchRouteDecision, SemanticSearchStorageProfile,
    SemanticSharingScope,
};

fn static_profile() -> SemanticSearchStorageProfile {
    SemanticSearchStorageProfile {
        mutation_class: SemanticMutationClass::Immutable,
        sharing_scope: SemanticSharingScope::ToolchainVersion,
        identity_digest: "blake3-256:toolchain".to_owned(),
        algorithm_evidence: SemanticSearchAlgorithmEvidence {
            tree_depth: 9,
            traversal_radius: 4,
            graph_node_count: 10_000,
            graph_edge_count: 24_000,
            merkle_delta_owner_count: 0,
            dependency_fan_out: 42,
            cross_workspace_reuse_count: 20,
            component_count: Some(3),
            cut_vertex_count: Some(7),
            provider_graph_evidence: vec![ProviderGraphEvidence {
                provider_id: "asp-python".to_owned(),
                algorithm_id: "scipy-csgraph-components".to_owned(),
                evidence_digest: "blake3-256:provider-graph".to_owned(),
            }],
        },
    }
}

#[test]
fn route_contract_keeps_schema_version_out_of_rust_type_names() {
    let decision = SemanticSearchRouteDecision::static_database(static_profile())
        .expect("valid static database route");

    assert_eq!(decision.schema_id, SEMANTIC_SEARCH_STORAGE_ROUTE_SCHEMA_ID);
    decision
        .validate_database_route()
        .expect("explicit static route admits DB adapter");
}

#[test]
fn database_adapter_contract_rejects_resident_route() {
    let decision = SemanticSearchRouteDecision::resident_memory(static_profile())
        .expect("internally consistent resident route");

    assert!(decision.validate_database_route().is_err());
}

#[test]
fn serialized_route_decision_matches_shared_schema() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-search-storage-route.v1.schema.json"
    ))
    .expect("storage route schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("compile storage route schema");
    let decision = SemanticSearchRouteDecision::static_database(static_profile())
        .expect("valid static database route");
    let instance = serde_json::to_value(decision).expect("serialize storage route decision");

    assert!(validator.is_valid(&instance));
}
