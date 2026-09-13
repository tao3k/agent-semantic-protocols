// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    AspClientWorkspaceQueryPlaybookRequest, admit_cold_query_owner_paths,
    bind_query_materialization_to_request, emit_runtime_search_trace_observation,
    materialize_query_playbook_receipt, query_playbook_generation_provider_targets,
    record_settled_client_timing_observations, runtime_search_trace_budget_micros,
    workspace_query_materialization_key,
};
use agent_semantic_content_identity::content_binding::{
    AuthorityStamp, ContentBinding, ContentIdentity, ContentPublicationCommit,
};
use agent_semantic_content_identity::runtime_execution::{
    RuntimeExecutionBinding, RuntimeExecutionBindingInput,
};
use agent_semantic_content_identity::runtime_workspace_execution_publication::{
    RuntimeWorkspaceExecutionPublication, RuntimeWorkspaceExecutionPublicationInput,
};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn runtime_binding() -> RuntimeExecutionBinding {
    let project_workspace = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("Project Workspace");
    let identity = ContentIdentity {
        runtime_artifact_digest: digest('1'),
        workspace_snapshot_digest: digest('2'),
        source_generation_digest: digest('1'),
        source_index_digest: digest('4'),
        schema_digest: digest('5'),
        provider_catalog_digest: digest('2'),
    };
    let content_binding = ContentBinding::new(
        identity.clone(),
        AuthorityStamp {
            key_id: "runtime-server-complete-generation".into(),
            canonical_digest: identity.digest(),
            signature: digest('7'),
        },
    )
    .expect("content binding");
    RuntimeExecutionBinding::new(RuntimeExecutionBindingInput {
        project_workspace,
        worktree_instance_id: "worktree-main".into(),
        publication_nonce: "publication-1".into(),
        content_binding,
        runtime_artifact_digest: digest('1').into(),
        evaluator_policy_digest: digest('8').into(),
        active_artifact_receipt_digest: digest('9').into(),
        evaluator_abi_digest: digest('a').into(),
    })
    .expect("Runtime execution binding")
}

fn params() -> AspClientWorkspaceQueryPlaybookRequest {
    AspClientWorkspaceQueryPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request".into(),
        schema_version: "1".into(),
        language: Some("rust".into()),
        documents: Some("org".into()),
        selectors: vec![
            "org://docs/publication.org#item/heading/Publication".into(),
            "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry".into(),
        ],
        projection: "source".into(),
    }
}

fn execution_publication() -> RuntimeWorkspaceExecutionPublication {
    let runtime_execution_binding = runtime_binding();
    let content_publication_commit = ContentPublicationCommit::linearize(
        runtime_execution_binding.content_binding.identity.clone(),
        runtime_execution_binding
            .content_binding
            .authority_stamp
            .clone(),
    )
    .expect("content publication commit");
    RuntimeWorkspaceExecutionPublication::new(RuntimeWorkspaceExecutionPublicationInput {
        workspace_identity: "workspace-23cc5ba784c605ae".into(),
        generation_digest: digest('1').into(),
        source_root_digest: digest('2').into(),
        content_publication_commit,
        runtime_execution_binding,
        runtime_bundle_digest: digest('f').into(),
    })
    .expect("workspace execution publication")
}

#[tokio::test]
async fn cold_query_rejects_an_impossible_owner_before_generation_admission() {
    let root = tempfile::tempdir().expect("cold Query workspace");
    let error = admit_cold_query_owner_paths(
        root.path(),
        &["rust://src/missing.rs#item/function/missing".to_owned()],
    )
    .await
    .expect_err("missing exact owner must fail before cold generation");
    let super::AspClientOperationError::Terminal(error) = error else {
        panic!("cold owner rejection must be typed");
    };
    assert_eq!(error.reason_kind, "query-playbook-owner-missing");
}

#[test]
fn query_provider_targets_fail_closed_before_generation_admission() {
    let error = query_playbook_generation_provider_targets(
        &["rust://src/lib.rs#item/function/missing".to_owned()],
        &[],
    )
    .expect_err("uninstalled selector producer must fail before cold generation");
    let super::AspClientOperationError::Terminal(error) = error else {
        panic!("provider rejection must be typed");
    };
    assert_eq!(error.reason_kind, "query-playbook-provider-not-installed");
}

#[test]
fn query_materialization_identity_binds_semantics_and_rebinds_only_envelope_request_id() {
    let request = params();
    let first = workspace_query_materialization_key(&request, "blake3-256:generation-a")
        .unwrap_or_else(|_| panic!("first Query materialization key"));
    let repeated = workspace_query_materialization_key(&request, "blake3-256:generation-a")
        .unwrap_or_else(|_| panic!("repeated Query materialization key"));
    let next_generation = workspace_query_materialization_key(&request, "blake3-256:generation-b")
        .unwrap_or_else(|_| panic!("next-generation Query materialization key"));
    let mut changed = request.clone();
    changed.projection = "callable-skeleton".to_owned();
    let next_projection = workspace_query_materialization_key(&changed, "blake3-256:generation-a")
        .unwrap_or_else(|_| panic!("next-projection Query materialization key"));
    assert_eq!(first, repeated);
    assert_ne!(first, next_generation);
    assert_ne!(first, next_projection);

    let template = serde_json::json!({
        "requestId": first,
        "requestedSelectors": request.selectors,
        "terminal": {"state": "ready"}
    });
    let rebound = bind_query_materialization_to_request(&template, "request-current")
        .unwrap_or_else(|_| panic!("bind current request identity"));
    assert_eq!(rebound["requestId"], "request-current");
    assert_eq!(
        rebound["requestedSelectors"],
        template["requestedSelectors"]
    );
    assert_eq!(rebound["terminal"], template["terminal"]);
}

#[test]
fn client_timing_is_settled_once_against_the_multilanguage_execution_publication() {
    let publication = execution_publication();
    let witness = agent_semantic_client_protocol::RuntimeSearchClientTimingWitness::new(
        "session-1",
        "request-1",
        [11, 22, 33],
    )
    .expect("client timing witness");
    let mut bus = agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus::new();
    let selector_params = params();
    let trace = record_settled_client_timing_observations(
        &publication,
        &witness,
        "session-1",
        "request-1",
        vec!["org".into(), "rust".into()],
        vec!["asp-org".into(), "asp-rust".into()],
        Some(&selector_params.projection),
        &bus.sender,
    )
    .expect("settled client timing observations");

    assert_eq!(trace.observations().len(), 3);
    let observations = std::array::from_fn::<_, 3, _>(|_| {
        bus.receiver
            .try_recv()
            .expect("one client timing event")
            .into_observation()
    });
    assert_eq!(
        observations
            .each_ref()
            .map(|observation| observation.stage.as_str()),
        ["launcher", "client-frame-encode", "ipc-connect"]
    );
    let generation_digest = digest('1');
    let runtime_artifact_digest = digest('1');
    for observation in observations {
        assert_eq!(
            observation.workspace_identity.as_deref(),
            Some("workspace-23cc5ba784c605ae")
        );
        assert_eq!(observation.operation_id.as_deref(), Some("request-1"));
        assert_eq!(
            observation.language_ids,
            Some(vec!["org".into(), "rust".into()])
        );
        assert_eq!(
            observation.provider_ids,
            Some(vec!["asp-org".into(), "asp-rust".into()])
        );
        assert_eq!(
            observation.generation_digest.as_deref(),
            Some(generation_digest.as_str())
        );
        assert_eq!(
            observation.runtime_artifact_digest.as_deref(),
            Some(runtime_artifact_digest.as_str())
        );
        assert_eq!(observation.requested_projection.as_deref(), Some("source"));
    }

    let foreign = agent_semantic_client_protocol::RuntimeSearchClientTimingWitness::new(
        "session-1",
        "request-foreign",
        [1, 2, 3],
    )
    .expect("foreign timing witness");
    assert_eq!(
        record_settled_client_timing_observations(
            &publication,
            &foreign,
            "session-1",
            "request-1",
            vec!["org".into(), "rust".into()],
            vec!["asp-org".into(), "asp-rust".into()],
            Some(&selector_params.projection),
            &bus.sender,
        )
        .expect_err("foreign request must fail before telemetry admission"),
        "runtime-search-client-timing-identity-mismatch"
    );
    assert!(bus.receiver.try_recv().is_err());

    emit_runtime_search_trace_observation(
        &bus.sender,
        trace.record_server_admission_queue(5, runtime_search_trace_budget_micros()),
    );
    emit_runtime_search_trace_observation(
        &bus.sender,
        trace.record_snapshot_resolve(8, runtime_search_trace_budget_micros()),
    );
    let binding = &publication.runtime_execution_binding;
    let receipt = materialize_query_playbook_receipt(
        "request-1",
        &selector_params,
        binding,
        publication.publication_digest.as_str(),
        publication.runtime_bundle_digest.as_str(),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        Some((&trace, &bus.sender)),
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('1'),
                root_digest: digest('2'),
                resolved_selector: selector.to_owned(),
                bytes: format!("materialized:{selector}").into_bytes(),
            })
        },
    )
    .unwrap_or_else(|_| panic!("runtime-bound Query materialization"));
    assert_eq!(receipt["terminal"]["state"], "ready");
    let server_stages = std::array::from_fn::<_, 5, _>(|_| {
        bus.receiver
            .try_recv()
            .expect("one server timing event")
            .into_observation()
            .stage
    });
    assert_eq!(
        server_stages,
        [
            "server-admission-queue",
            "snapshot-resolve",
            "provider-dispatch",
            "parse-index-query",
            "projection-rank",
        ]
    );
    assert_eq!(trace.observations().len(), 8);
}

#[test]
fn query_playbook_materializes_one_runtime_bound_terminal_in_selector_order() {
    let binding = runtime_binding();
    let receipt = materialize_query_playbook_receipt(
        "request-query-playbook",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('1'),
                root_digest: digest('2'),
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
}

#[test]
fn query_playbook_selector_failure_exposes_no_partial_materialization() {
    let binding = runtime_binding();
    let receipt = materialize_query_playbook_receipt(
        "request-query-playbook-failed",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            if selector.starts_with("org://") {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('1'),
                    root_digest: digest('2'),
                    resolved_selector: selector.to_owned(),
                    bytes: b"publication".to_vec(),
                })
            } else {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::OwnerMissing {
                    generation_digest: digest('1'),
                    root_digest: digest('2'),
                })
            }
        },
    )
    .unwrap_or_else(|_| panic!("typed failed materialization terminal"));
    assert_eq!(receipt["terminal"]["state"], "failed");
    assert_eq!(receipt["terminal"]["terminalCount"], 1);
    assert_eq!(
        receipt["terminal"]["reasonKind"],
        "query-playbook-selector-not-materialized"
    );
    assert_eq!(receipt["materializations"], serde_json::json!([]));
}

#[test]
fn query_playbook_selector_read_error_emits_one_failed_terminal_without_partial_materialization() {
    let binding = runtime_binding();
    let receipt = materialize_query_playbook_receipt(
        "request-query-playbook-read-failed",
        &params(),
        &binding,
        &digest('e'),
        &digest('f'),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        None,
        |_projection, selector| {
            if selector.starts_with("org://") {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('1'),
                    root_digest: digest('2'),
                    resolved_selector: selector.to_owned(),
                    bytes: b"must-not-escape".to_vec(),
                })
            } else {
                Err("resident selector reader failed".to_owned())
            }
        },
    )
    .unwrap_or_else(|_| panic!("typed failed materialization terminal"));
    assert_eq!(receipt["terminal"]["state"], "failed");
    assert_eq!(receipt["terminal"]["terminalCount"], 1);
    assert_eq!(
        receipt["terminal"]["reasonKind"],
        "query-playbook-selector-read-failed"
    );
    assert_eq!(receipt["materializations"], serde_json::json!([]));
}
