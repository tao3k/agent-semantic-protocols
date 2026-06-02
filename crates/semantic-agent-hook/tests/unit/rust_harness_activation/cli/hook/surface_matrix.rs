use serde_json::{Value, json};

use crate::rust_harness_activation::support::temp_project_root;

use super::support::run_hook_decision;

struct SurfaceCase {
    name: &'static str,
    payload: Value,
    decision: &'static str,
    reason_kind: &'static str,
    provider_id: Option<&'static str>,
    route_kind: Option<&'static str>,
    selector: Option<&'static str>,
    route_has_code: bool,
}

#[test]
fn cli_hook_surface_matrix_covers_read_wrappers_nested_encoded_globs_and_non_source() {
    let cases = vec![
        SurfaceCase {
            name: "direct-read-exact-source",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "src/lib.rs"}
            }),
            decision: "deny",
            reason_kind: "direct-source-read",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "direct-read-source-range",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "src/lib.rs:1-2"}
            }),
            decision: "deny",
            reason_kind: "direct-source-read",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs:1-2"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "wrapper-read-range",
            payload: json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "rtk read src/lib.rs:1-2"}
            }),
            decision: "deny",
            reason_kind: "direct-source-read",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs:1-2"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "nested-parallel-wrapper-read",
            payload: json!({
                "tool_name": "multi_tool_use.parallel",
                "tool_input": {
                    "tool_uses": [
                        {
                            "recipient_name": "functions.exec_command",
                            "parameters": {"cmd": "rtk read src/lib.rs:1-2"}
                        }
                    ]
                }
            }),
            decision: "deny",
            reason_kind: "direct-source-read",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs:1-2"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "encoded-json-tool-input",
            payload: json!({
                "tool_name": "functions.exec_command",
                "tool_input": "{\"cmd\":\"sed -n '1,40p' src/lib.rs\"}"
            }),
            decision: "deny",
            reason_kind: "bulk-source-dump",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs:1:40"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "source-glob-prime-route",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "*.rs"}
            }),
            decision: "deny",
            reason_kind: "direct-source-read",
            provider_id: Some("rs-harness"),
            route_kind: Some("prime"),
            selector: None,
            route_has_code: false,
        },
        SurfaceCase {
            name: "path-only-tail-is-still-denied-without-fabricated-range",
            payload: json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "tail -40 src/lib.rs | head -n 10"}
            }),
            decision: "deny",
            reason_kind: "bulk-source-dump",
            provider_id: Some("rs-harness"),
            route_kind: Some("query"),
            selector: Some("src/lib.rs"),
            route_has_code: true,
        },
        SurfaceCase {
            name: "non-source-read-is-allowed",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "README.md"}
            }),
            decision: "allow",
            reason_kind: "none",
            provider_id: None,
            route_kind: None,
            selector: None,
            route_has_code: false,
        },
    ];

    for case in cases {
        let root = temp_project_root(case.name);
        let decision = run_hook_decision(&root, "pre-tool", case.payload);

        assert_eq!(decision["decision"], case.decision, "case={}", case.name);
        assert_eq!(
            decision["reasonKind"], case.reason_kind,
            "case={}",
            case.name
        );

        let Some(provider_id) = case.provider_id else {
            assert!(
                decision["routes"].as_array().is_none_or(Vec::is_empty),
                "case={} routes={:?}",
                case.name,
                decision["routes"]
            );
            continue;
        };

        assert_eq!(
            decision["routes"][0]["providerId"], provider_id,
            "case={}",
            case.name
        );
        if let Some(route_kind) = case.route_kind {
            assert_eq!(
                decision["routes"][0]["kind"], route_kind,
                "case={}",
                case.name
            );
        }

        let argv = decision["routes"][0]["argv"]
            .as_array()
            .expect("route argv");
        if let Some(selector) = case.selector {
            assert_eq!(
                decision["routes"][0]["argv"][5], selector,
                "case={}",
                case.name
            );
        } else {
            assert!(
                !argv.iter().any(|arg| arg == "--selector"),
                "case={} argv={argv:?}",
                case.name
            );
        }
        assert_eq!(
            argv.iter().any(|arg| arg == "--code"),
            case.route_has_code,
            "case={} argv={argv:?}",
            case.name
        );
    }
}
