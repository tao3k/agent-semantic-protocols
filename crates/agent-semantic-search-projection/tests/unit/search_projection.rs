use agent_semantic_search_projection::SearchProjectionDensityV1;
use agent_semantic_search_projection::SearchProjectionRenderer;
use agent_semantic_search_projection::SearchProjectionRequestV1;
use agent_semantic_search_projection::SemanticSearchPacketV1;
use agent_semantic_search_projection::TopologySearchProjectionRenderer;
use serde_json::json;

fn packet() -> SemanticSearchPacketV1 {
    SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "view": "owner",
        "query": "src/lib.rs",
        "items": [],
        "owners": [],
        "nextActions": []
    }))
    .expect("valid packet")
}

#[test]
fn density_does_not_change_semantic_digest() {
    let packet = packet();
    let renderer = TopologySearchProjectionRenderer;
    let terse = renderer
        .render(
            &packet,
            &SearchProjectionRequestV1::new("topology", SearchProjectionDensityV1::Terse),
        )
        .expect("terse projection");
    let expanded = renderer
        .render(
            &packet,
            &SearchProjectionRequestV1::new("topology", SearchProjectionDensityV1::Expanded),
        )
        .expect("expanded projection");

    assert_eq!(terse.semantic_digest(), expanded.semantic_digest());
    assert_ne!(terse.content(), expanded.content());
    assert!(terse.content().contains("density=terse"));
    assert!(expanded.content().contains("density=expanded"));
}

#[test]
fn request_rejects_undefined_fields() {
    let error = serde_json::from_value::<SearchProjectionRequestV1>(json!({
        "schemaId": "asp.search-projection-request.v1",
        "schemaVersion": "v1",
        "projectionId": "owner",
        "density": "standard",
        "unexpectedField": true
    }))
    .expect_err("undefined request fields must be rejected");

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn ranked_graph_packet_uses_shared_projection_renderer() {
    let value = json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-result",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-result",
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "entryNodeIds": ["query:parser"],
        "rankedNodes": [
            {
                "id": "query:parser",
                "kind": "query",
                "role": "term",
                "value": "parser",
                "action": "lexical"
            },
            {
                "id": "owner:cli",
                "kind": "owner",
                "role": "path",
                "value": "src/cli.rs",
                "action": "owner"
            }
        ],
        "edges": [
            {
                "source": "query:parser",
                "target": "owner:cli",
                "relation": "matches",
                "weight": 1.5
            }
        ],
        "scores": {
            "query:parser": 2.6,
            "owner:cli": 2.35
        },
        "typedPaths": [
            {
                "id": "P1",
                "source": "query:parser",
                "sink": "owner:cli",
                "pathKind": "constrained-shortest"
        }
        ]
    });
    let first =
        agent_semantic_search_projection::GraphTurboResultPacketV1::from_value(value.clone())
            .expect("typed graph result");
    let second = agent_semantic_search_projection::GraphTurboResultPacketV1::from_value(value)
        .expect("typed graph result");
    let request =
        SearchProjectionRequestV1::new("ranked-frontier", SearchProjectionDensityV1::Terse);
    let renderer = agent_semantic_search_projection::RankedFrontierSearchProjectionRenderer;
    let rendered = agent_semantic_search_projection::SearchProjectionRenderer::render(
        &renderer, &first, &request,
    )
    .expect("shared ranked frontier projection");

    assert_eq!(
        agent_semantic_search_projection::SearchProjectionSource::semantic_digest(&first),
        agent_semantic_search_projection::SearchProjectionSource::semantic_digest(&second)
    );
    assert_eq!(
        rendered.semantic_digest(),
        agent_semantic_search_projection::SearchProjectionSource::semantic_digest(&first)
    );
    assert!(rendered.content().contains("density=terse"));
    assert!(rendered.content().contains("I=owner:cli kind=owner"));
}

#[test]
fn topology_projects_typed_packages_policy_handles_and_prime_facts() {
    let workspace = SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "view": "workspace",
        "projectRoot": ".",
        "packages": [{"id": ".", "fields": {}}],
        "items": [],
        "owners": [],
        "nextActions": []
    }))
    .expect("workspace packet");
    let request = SearchProjectionRequestV1::new("topology", SearchProjectionDensityV1::Terse);
    let renderer = TopologySearchProjectionRenderer;
    let workspace = renderer
        .render(&workspace, &request)
        .expect("workspace projection");
    assert!(workspace.content().contains("P=package:pkg(.)!owner"));
    assert!(!workspace.content().contains("G>{}"));

    let policy = SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "view": "policy",
        "query": "RULE-001",
        "semanticHandles": [{
            "id": "RULE-001",
            "ownerPath": "src/rules.rs",
            "testPaths": ["tests/rules.rs"]
        }],
        "items": [],
        "owners": [],
        "nextActions": []
    }))
    .expect("policy packet");
    let policy = renderer
        .render(&policy, &request)
        .expect("policy projection");
    assert!(
        policy
            .content()
            .contains("O=owner:path(src/rules.rs)!owner")
    );
    assert!(
        policy
            .content()
            .contains("T=test:path(tests/rules.rs)!tests")
    );

    let owner_with_test = SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "typescript",
        "providerId": "asp-typescript",
        "view": "lexical",
        "query": "findOrderStatus",
        "items": [],
        "owners": [
            {"path": "tests/index.test.ts", "role": "test"},
            {"path": "src/index.ts", "role": "source"}
        ],
        "nextActions": []
    }))
    .expect("test owner packet");
    let owner_with_test = renderer
        .render(&owner_with_test, &request)
        .expect("test owner projection");
    assert!(
        owner_with_test
            .content()
            .contains("T=test:path(tests/index.test.ts)!tests")
    );
    assert!(
        owner_with_test
            .content()
            .contains("O=owner:path(src/index.ts)!owner")
    );

    let prime = SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "view": "prime",
        "projectRoot": ".",
        "notes": [
            {"kind": "feature", "message": "io-util enables=bytes"},
            {"kind": "cfg", "message": "feature:io-util declared_in=features"}
        ],
        "items": [],
        "owners": [{"path": "src/lib.rs"}],
        "nextActions": []
    }))
    .expect("prime packet");
    let prime = renderer.render(&prime, &request).expect("prime projection");
    assert!(
        prime
            .content()
            .contains("F=feature:feature(io-util)!features")
    );
    assert!(prime.content().contains("C=cfg:cfg(feature:io-util)!cfg"));
}

#[test]
fn topology_preserves_same_symbol_with_distinct_canonical_locators() {
    let packet = SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "view": "owner",
        "query": "src/lib.rs",
        "header": {"fields": {"itemQuery": "parse"}},
        "owners": [{"path": "src/lib.rs"}],
        "items": [
            {
                "name": "parse",
                "kind": "method",
                "fields": {
                    "structuralSelector": "rust://src/lib.rs#item/method/parse/scope/type/A"
                }
            },
            {
                "name": "parse",
                "kind": "method",
                "fields": {
                    "structuralSelector": "rust://src/lib.rs#item/method/parse/scope/type/B"
                }
            }
        ],
        "nextActions": []
    }))
    .expect("owner item packet");
    let rendered = TopologySearchProjectionRenderer
        .render(
            &packet,
            &SearchProjectionRequestV1::new("topology", SearchProjectionDensityV1::Terse),
        )
        .expect("owner item projection");

    assert!(
        rendered
            .content()
            .contains("rust://src/lib.rs#item/method/parse/scope/type/A")
    );
    assert!(
        rendered
            .content()
            .contains("rust://src/lib.rs#item/method/parse/scope/type/B")
    );
}
