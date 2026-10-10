// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    digest, failed_query_receipt, materialize_query_playbook_receipt, params, runtime_binding,
};

#[test]
fn query_playbook_materializes_one_runtime_bound_terminal_in_selector_order() {
    let binding = runtime_binding();
    let receipt = materialize_query_playbook_receipt(
        "request-query-playbook",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &digest('1'),
        &"2".repeat(64),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('1'),
                root_digest: "2".repeat(64),
                resolved_selector: selector.to_owned(),
                bytes: format!("materialized:{selector}").into_bytes(),
            })
        },
    )
    .unwrap_or_else(|_| panic!("one complete materialization terminal"));
    assert_eq!(receipt["terminal"]["state"], "ready");
    assert_eq!(receipt["terminal"]["terminalCount"], 1);
    assert_eq!(receipt["materializations"].as_array().unwrap().len(), 2);
    assert_eq!(
        receipt["materializations"][0]["selector"],
        params().selectors[0]
    );
    assert_eq!(
        receipt["runtimeExecutionBinding"],
        serde_json::to_value(binding).unwrap()
    );
    assert_eq!(receipt["sourceGenerationDigest"], digest('1'));
    assert_eq!(receipt["sourceRootDigest"], "2".repeat(64));
}

#[test]
fn query_playbook_uses_the_exact_read_generation_after_parser_materialization() {
    let binding = runtime_binding();
    let receipt = materialize_query_playbook_receipt(
        "request-query-playbook-parser-generation",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &digest('1'),
        &"2".repeat(64),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('7'),
                root_digest: "2".repeat(64),
                resolved_selector: selector.to_owned(),
                bytes: format!("materialized:{selector}").into_bytes(),
            })
        },
    )
    .unwrap_or_else(|_| panic!("parser-materialized exact generation"));
    assert_eq!(receipt["terminal"]["state"], "ready");
    assert_eq!(receipt["sourceGenerationDigest"], digest('7'));
    assert_eq!(receipt["sourceRootDigest"], "2".repeat(64));
}

#[test]
fn query_playbook_rejects_a_selector_read_from_another_content_generation() {
    let binding = runtime_binding();
    let (reason_kind, receipt) = failed_query_receipt(materialize_query_playbook_receipt(
        "request-query-playbook-drift",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &digest('1'),
        &"2".repeat(64),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: if selector.starts_with("org://") {
                    digest('1')
                } else {
                    digest('9')
                },
                root_digest: "2".repeat(64),
                resolved_selector: selector.to_owned(),
                bytes: b"content-bound".to_vec(),
            })
        },
    ));
    assert_eq!(reason_kind, "query-playbook-content-identity-mismatch");
    assert_eq!(receipt["terminal"]["state"], "failed");
    assert_eq!(
        receipt["terminal"]["reasonKind"],
        "query-playbook-content-identity-mismatch"
    );
    assert_eq!(receipt["materializations"], serde_json::json!([]));
}
