use agent_semantic_protocol::exact_projection::render_callable_skeleton;

#[test]
fn callable_skeleton_renderer_uses_root_relative_selector_references() {
    let payload = serde_json::json!({
        "schemaId": "agent.semantic-protocols.callable-skeleton-projection",
        "schemaVersion": "1",
        "languageId": "python",
        "rootSelector": {
            "selector": "python://src/main.py#item/function/run"
        },
        "callable": {
            "displayName": "run λ"
        },
        "nodes": [
            {
                "nodeId": "callable:root",
                "kind": "callable",
                "label": "run",
                "order": 0,
                "exactSelector": {
                    "selector": "python://src/main.py#item/function/run",
                    "generationIdentityDigest": "generation-digest"
                }
            },
            {
                "nodeId": "branch:1",
                "kind": "branch",
                "label": "if 条件",
                "order": 1,
                "exactSelector": {
                    "selector": "python://src/main.py#item/function/run/segment/branch/ordinal-1",
                    "generationIdentityDigest": "generation-digest"
                }
            }
        ],
        "cost": {
            "sourceBytes": 1024
        }
    });

    let rendered = render_callable_skeleton(&payload).expect("render projection");
    assert!(rendered.contains("language=python"));
    assert!(rendered.contains("callable=\"run λ\""));
    assert!(rendered.contains("R=python://src/main.py#item/function/run"));
    assert!(rendered.contains("label=\"if 条件\""));
    assert!(rendered.contains("selector=R/segment/branch/ordinal-1"));
    assert!(!rendered.contains("generation-digest"));
    assert!(rendered.contains("omit=source,node-authority-copies,expanded-relations"));
    assert!(rendered.len() < serde_json::to_vec(&payload).expect("encode payload").len());
    assert!(rendered.contains(&format!("renderedBytes={}", rendered.len())));
}

#[test]
fn callable_skeleton_renderer_rejects_prefixed_schema_version() {
    let payload = serde_json::json!({
        "schemaId": "agent.semantic-protocols.callable-skeleton-projection",
        "schemaVersion": "v1"
    });

    let error = render_callable_skeleton(&payload)
        .expect_err("schemaVersion must use the shared numeric string");
    assert!(error.contains("unsupported schema identity"));
}
