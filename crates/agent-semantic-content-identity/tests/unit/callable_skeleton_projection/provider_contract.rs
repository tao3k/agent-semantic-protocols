use super::CallableSkeletonProjectionV1;

#[test]
fn python_provider_projection_deserializes_through_shared_contract() {
    let projection = serde_json::json!({
        "schemaId": "agent.semantic-protocols.callable-skeleton-projection",
        "schemaVersion": "1",
        "projectionKind": "callable-skeleton",
        "languageId": "python",
        "providerId": "py-harness",
        "rootSelector": {
            "schemaId": "asp.exact-structural-selector.v1",
            "schemaVersion": "1",
            "languageId": "python",
            "ownerPath": "fixture.py",
            "selector": "python://fixture.py#item/function/f",
            "generationIdentityDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "parserIdentityDigest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "queryPackDigest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "rootItemSelector": {
                "schemaId": "asp.canonical-item-selector.v1",
                "schemaVersion": "1",
                "languageId": "python",
                "kind": "function",
                "symbol": "f",
                "scopes": [],
                "structuralSelector": "python://fixture.py#item/function/f"
            },
            "segments": []
        },
        "rootNodeId": "callable:root",
        "callable": {"kind": "function", "displayName": "f", "signature": "f"},
        "nodes": [{
            "nodeId": "callable:root",
            "kind": "callable",
            "label": "f",
            "order": 0,
            "queryable": true,
            "exactSelector": {
                "schemaId": "asp.exact-structural-selector.v1",
                "schemaVersion": "1",
                "languageId": "python",
                "ownerPath": "fixture.py",
                "selector": "python://fixture.py#item/function/f",
                "generationIdentityDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "parserIdentityDigest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "queryPackDigest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "rootItemSelector": {
                    "schemaId": "asp.canonical-item-selector.v1",
                    "schemaVersion": "1",
                    "languageId": "python",
                    "kind": "function",
                    "symbol": "f",
                    "scopes": [],
                    "structuralSelector": "python://fixture.py#item/function/f"
                },
                "segments": []
            },
            "languageFacts": {"async": false, "decoratorCount": 0, "inputCount": 0}
        }],
        "relations": [],
        "cost": {"sourceBytes": 21, "projectedBytes": 21, "omittedBytes": 0},
        "languageFacts": {"parser": "ast", "syntax": "python"}
    });
    let decoded: CallableSkeletonProjectionV1 =
        serde_json::from_value(projection).expect("Python projection must match shared schema");
    assert_eq!(decoded.language_id, "python");
    assert_eq!(decoded.provider_id, "py-harness");
}
