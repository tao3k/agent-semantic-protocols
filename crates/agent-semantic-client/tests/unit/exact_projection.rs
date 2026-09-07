// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client::exact_projection::render_callable_skeleton;
use serde_json::Value;
use serde_json::json;

fn envelope(payload: Value, language_id: &str, root_selector: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-projection",
        "schemaVersion": "1",
        "projectionKind": "callable-skeleton",
        "languageId": language_id,
        "providerId": format!("asp-{language_id}"),
        "rootSelector": root_selector,
        "evidenceContextRef": format!("blake3-256:{}", "e".repeat(64)),
        "payloadSchemaId": "agent.semantic-protocols.callable-skeleton",
        "payloadDigest": format!("blake3-256:{}", "0".repeat(64)),
        "payload": payload
    })
}

#[test]
fn callable_skeleton_renderer_uses_root_relative_selector_references() {
    let payload = serde_json::json!({
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
            },
            {
                "nodeId": "branch:1",
                "kind": "branch",
                "label": "if 条件",
                "order": 1,
                "selector": "python://src/main.py#item/function/run/segment/branch/ordinal-1"
            }
        ],
        "cost": {
            "sourceBytes": 1024
        }
    });

    let rendered = render_callable_skeleton(&envelope(
        payload.clone(),
        "python",
        "python://src/main.py#item/function/run",
    ))
    .expect("render projection");
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
fn callable_skeleton_renderer_consumes_runtime_selector_references() {
    let payload = serde_json::json!({
        "rootSelector": {
            "schemaId": "asp.exact-structural-selector-reference.v1",
            "schemaVersion": "1",
            "languageId": "rust",
            "selector": "rust://src/lib.rs#item/function/run",
            "evidenceContextRef": format!("blake3-256:{}", "a".repeat(64))
        },
        "callable": { "displayName": "run" },
        "nodes": [
            {
                "nodeId": "callable:root",
                "kind": "callable",
                "label": "run",
                "order": 0,
                "queryable": false
            },
            {
                "nodeId": "branch:1",
                "kind": "branch",
                "label": "if",
                "order": 1,
                "queryable": true,
                "selectorRef": "$root/segment/branch/ordinal-1"
            }
        ],
        "cost": { "sourceBytes": 2048 }
    });

    let rendered = render_callable_skeleton(&envelope(
        payload,
        "rust",
        "rust://src/lib.rs#item/function/run",
    ))
    .expect("render referenced projection");
    assert!(rendered.contains("selector=R/segment/branch/ordinal-1"));
    assert!(!rendered.contains("evidenceContextRef"));
    assert!(!rendered.contains("blake3-256"));
}

#[test]
fn callable_skeleton_renderer_rejects_prefixed_schema_version() {
    let payload = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-projection",
        "schemaVersion": "v1"
    });

    let error = render_callable_skeleton(&payload)
        .expect_err("schemaVersion must use the shared numeric string");
    assert!(error.contains("unsupported schema identity"));
}
