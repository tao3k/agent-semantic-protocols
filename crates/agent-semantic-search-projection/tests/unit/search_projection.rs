use agent_semantic_search_projection::{
    SearchProjectionDensityV1, SearchProjectionRenderer, SearchProjectionRequestV1,
    SemanticSearchPacketV1, TopologySearchProjectionRenderer,
};
use serde_json::json;

fn packet() -> SemanticSearchPacketV1 {
    SemanticSearchPacketV1::from_value(json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
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

    assert_eq!(terse.semantic_digest, expanded.semantic_digest);
    assert_ne!(terse.content, expanded.content);
    assert!(terse.content.contains("density=terse"));
    assert!(expanded.content.contains("density=expanded"));
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
        "seedIds": ["query:parser"],
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
        rendered.semantic_digest,
        agent_semantic_search_projection::SearchProjectionSource::semantic_digest(&first)
    );
    assert!(rendered.content.contains("density=terse"));
    assert!(rendered.content.contains("I=owner:cli kind=owner"));
}
