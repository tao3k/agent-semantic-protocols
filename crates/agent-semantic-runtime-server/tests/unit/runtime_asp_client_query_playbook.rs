// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest, admit_cold_query_owner_paths,
    bind_query_materialization_to_request, durable_exact_execution_matches_current,
    durable_projection_is_direct, emit_runtime_search_trace_observation,
    materialize_query_playbook_receipt, process_cold_owner_content_digest,
    query_playbook_generation_provider_targets, read_query_projection_handoff,
    record_settled_client_timing_observations, resident_exact_query_projections,
    runtime_search_trace_budget_micros, try_process_cold_exact_owner_replay,
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

#[path = "runtime_asp_client_query_playbook_fixtures.rs"]
mod fixtures;
use fixtures::{
    execution_publication_for_exact_generation, process_cold_generation, process_cold_resources,
};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn failed_query_receipt(
    result: Result<serde_json::Value, AspClientOperationError>,
) -> (String, serde_json::Value) {
    let error = match result {
        Err(error) => error,
        Ok(receipt) => {
            panic!("failed Query materialization must not publish a ready value: {receipt}")
        }
    };
    match error {
        AspClientOperationError::Terminal(error) => (
            error.reason_kind,
            error
                .details
                .expect("typed Query failure must retain the admitted receipt"),
        ),
        AspClientOperationError::Message(message) => {
            panic!("Query materialization failure must be typed: {message}")
        }
    }
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

#[test]
fn process_cold_owner_digest_streams_content_and_fails_closed_when_missing() {
    let root = tempfile::tempdir().expect("process-cold owner digest workspace");
    let owner = root.path().join("owner.rs");
    std::fs::write(&owner, b"fn owner() {}\n").expect("write process-cold owner");
    let (digest, byte_len) = match process_cold_owner_content_digest(&owner) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => panic!("existing owner must produce a digest"),
        Err(_) => panic!("existing owner digest must succeed"),
    };
    assert_eq!(
        digest,
        format!("blake3-256:{}", blake3::hash(b"fn owner() {}\n").to_hex())
    );
    assert_eq!(byte_len, b"fn owner() {}\n".len());

    std::fs::remove_file(&owner).expect("remove process-cold owner");
    match process_cold_owner_content_digest(&owner) {
        Ok(None) => {}
        Ok(Some(_)) => panic!("missing owner must not produce a digest"),
        Err(_) => panic!("missing owner must be a cache rejection, not an operation failure"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn process_cold_exact_owner_replay_materializes_a_real_durable_generation_on_first_call() {
    use agent_semantic_client_db::runtime_server_workspace::{
        RuntimeServerWorkspaceRegistry, RuntimeWorkspaceExecutionPublicationStore,
        WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
        WorkspaceRecoverySource, workspace_generation_pointer_path,
    };

    let temporary = tempfile::tempdir().expect("process-cold Query workspace");
    let project_root = temporary.path().join("project");
    std::fs::create_dir_all(project_root.join("src")).expect("create owner directory");
    let source = b"pub fn exact_owner() -> usize { 23 }\n";
    std::fs::write(project_root.join("src/lib.rs"), source).expect("write exact owner");
    let workspace_identity = "workspace-process-cold-query";
    let selector = "rust://src/lib.rs#item/function/exact_owner";
    let workspace_store_root = temporary.path().join("workspace-store");
    let registry = RuntimeServerWorkspaceRegistry::new(workspace_store_root.clone())
        .expect("process-cold registry");
    let recovery = registry
        .publish(
            "process-cold-generation",
            WorkspaceRecoverySource::TursoGeneration,
            process_cold_generation(&project_root, workspace_identity, selector, source),
        )
        .await
        .expect("publish process-cold generation");

    let project_workspace = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/process-cold",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("process-cold Project Workspace");
    let host_workspace = agent_semantic_content_identity::HostWorkspaceInitializationBinding::new(
        project_workspace.clone(),
        "worktree-main",
    )
    .expect("process-cold Host workspace");
    let runtime_bundle_digest = digest('f');
    let pointer_path =
        workspace_generation_pointer_path(&workspace_store_root, workspace_identity, &project_root)
            .expect("process-cold generation pointer");
    let WorkspaceExactProjectionDataPlaneOpen::Ready(exact) =
        WorkspaceExactProjectionDataPlaneClient::open_state(&pointer_path)
            .await
            .expect("open process-cold exact generation")
    else {
        panic!("published process-cold generation must have an exact data plane");
    };
    let exact_generation_digest = exact.generation_digest();
    let exact_root_digest = exact.root_digest();
    let owner_metadata = exact
        .owner_content_metadata("src/lib.rs")
        .expect("read exact owner metadata")
        .expect("published exact owner metadata");
    assert_eq!(
        owner_metadata.content_digest,
        format!("blake3-256:{}", blake3::hash(source).to_hex())
    );
    assert_eq!(owner_metadata.byte_len, source.len());
    assert_eq!(
        exact
            .direct_projection_byte_len(
                agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                selector,
            )
            .expect("read direct projection byte length"),
        Some(source.len())
    );
    assert_eq!(exact_generation_digest, recovery.generation_digest);
    let publication_root_digest =
        agent_semantic_search::canonical_blake3_digest(&exact_root_digest)
            .expect("canonical process-cold publication root digest");
    let publication = execution_publication_for_exact_generation(
        workspace_identity,
        &exact_generation_digest,
        &publication_root_digest,
        &runtime_bundle_digest,
        project_workspace,
    );
    let execution_root = pointer_path.parent().expect("process-cold execution root");
    RuntimeWorkspaceExecutionPublicationStore::open(execution_root)
        .await
        .expect("open process-cold execution store")
        .publish(&publication)
        .await
        .expect("publish process-cold execution binding");

    let request = AspClientWorkspaceQueryPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request".into(),
        schema_version: "1".into(),
        language: Some("rust".into()),
        documents: None,
        selectors: vec![selector.to_owned()],
        projection: "source".into(),
    };
    let initialized = super::InitializedWorkspace {
        project_root,
        host_workspace,
    };
    let (resource_supervisor, task_scope) = process_cold_resources();
    let result = try_process_cold_exact_owner_replay(
        "request-process-cold",
        workspace_identity,
        &request,
        &initialized,
        &workspace_store_root,
        &runtime_bundle_digest,
        &resource_supervisor,
        &task_scope,
        &[("rust".to_owned(), "asp-rust".to_owned())],
        tokio::time::Instant::now(),
    )
    .await;
    let payload = match result {
        Ok(Some(payload)) => payload,
        Ok(None) => panic!("matching durable generation must replay on the first Query call"),
        Err(AspClientOperationError::Message(message)) => {
            panic!("matching durable generation replay must succeed: {message}")
        }
        Err(AspClientOperationError::Terminal(error)) => panic!(
            "matching durable generation replay must succeed: reasonKind={} message={}",
            error.reason_kind, error.message
        ),
    };
    let receipt = serde_json::to_value(payload).expect("serialize process-cold Query receipt");
    assert_eq!(receipt["requestId"], "request-process-cold");
    assert_eq!(receipt["requestProfile"], "materialized");
    assert_eq!(receipt["terminal"]["state"], "ready");
    assert_eq!(receipt["sourceGenerationDigest"], exact_generation_digest);
    assert_eq!(receipt["sourceRootDigest"], exact_root_digest);
    assert_eq!(receipt["materializations"][0]["selector"], selector);
    const QUALIFICATION_SAMPLE_COUNT: usize = 128;
    let mut request_plane_samples = vec![
        receipt["requestPlaneElapsedMicros"]
            .as_u64()
            .expect("process-cold request-plane timing"),
    ];
    for sample in 1..QUALIFICATION_SAMPLE_COUNT {
        let repeated = try_process_cold_exact_owner_replay(
            &format!("request-process-cold-sample-{sample}"),
            workspace_identity,
            &request,
            &initialized,
            &workspace_store_root,
            &runtime_bundle_digest,
            &resource_supervisor,
            &task_scope,
            &[("rust".to_owned(), "asp-rust".to_owned())],
            tokio::time::Instant::now(),
        )
        .await;
        let payload = match repeated {
            Ok(Some(payload)) => payload,
            _ => panic!("every process-cold qualification sample must replay exactly"),
        };
        let receipt = serde_json::to_value(payload).expect("serialize repeated Query receipt");
        request_plane_samples.push(
            receipt["requestPlaneElapsedMicros"]
                .as_u64()
                .expect("repeated process-cold request-plane timing"),
        );
    }
    request_plane_samples.sort_unstable();
    let percentile = |percent: usize| {
        let rank = request_plane_samples
            .len()
            .saturating_mul(percent)
            .div_ceil(100);
        request_plane_samples[rank.saturating_sub(1)]
    };
    println!(
        "[scenario-benchmark] id=runtime-query-process-cold-exact-owner-replay sampleCount={} p50Micros={} p95Micros={} p99Micros={} pipelineTaskCount=1 ownerSnapshotCopyCount=0 providerProcessCount=0 fullGenerationAdmissionCount=0",
        request_plane_samples.len(),
        percentile(50),
        percentile(95),
        percentile(99),
    );

    std::fs::write(
        initialized.project_root.join("README.md"),
        b"unrelated workspace mutation\n",
    )
    .expect("write unrelated workspace file");
    assert!(matches!(
        try_process_cold_exact_owner_replay(
            "request-process-cold-unrelated-change",
            workspace_identity,
            &request,
            &initialized,
            &workspace_store_root,
            &runtime_bundle_digest,
            &resource_supervisor,
            &task_scope,
            &[("rust".to_owned(), "asp-rust".to_owned())],
            tokio::time::Instant::now(),
        )
        .await,
        Ok(Some(_))
    ));

    std::fs::write(
        initialized.project_root.join("src/lib.rs"),
        b"pub fn exact_owner() -> usize { 24 }\n",
    )
    .expect("mutate requested owner");
    assert!(matches!(
        try_process_cold_exact_owner_replay(
            "request-process-cold-owner-change",
            workspace_identity,
            &request,
            &initialized,
            &workspace_store_root,
            &runtime_bundle_digest,
            &resource_supervisor,
            &task_scope,
            &[("rust".to_owned(), "asp-rust".to_owned())],
            tokio::time::Instant::now(),
        )
        .await,
        Ok(None)
    ));
    assert_eq!(resource_supervisor.active_background_cpu(), 0);
    assert_eq!(resource_supervisor.active_memory_bytes(), 0);
    let lifecycle = task_scope.finish(0).expect("process-cold pipeline drained");
    assert_eq!(
        lifecycle.started,
        u64::try_from(QUALIFICATION_SAMPLE_COUNT + 2).expect("qualification task count")
    );
    assert_eq!(lifecycle.started, lifecycle.completed);
    assert_eq!(lifecycle.active, 0);
    assert_eq!(lifecycle.leaked, 0);
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
fn query_materialization_identity_binds_semantics_and_rebinds_request_local_witnesses() {
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
    let rebound = bind_query_materialization_to_request(
        std::sync::Arc::new(template.clone()),
        "request-current",
        "resident-hit",
        tokio::time::Instant::now(),
    )
    .unwrap_or_else(|_| panic!("bind current request identity"));
    let rebound = serde_json::to_value(rebound).expect("serialize rebound response");
    assert_eq!(rebound["requestId"], "request-current");
    assert_eq!(rebound["requestProfile"], "resident-hit");
    assert!(rebound["requestPlaneElapsedMicros"].is_u64());
    assert_eq!(
        rebound["requestedSelectors"],
        template["requestedSelectors"]
    );
    assert_eq!(rebound["terminal"], template["terminal"]);
}

#[test]
fn resident_exact_query_skips_owner_materialization_only_when_every_projection_resolves() {
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

    let request = params();
    let mut resident_reads = 0usize;
    let resident = resident_exact_query_projections(&request, |_projection, selector| {
        resident_reads += 1;
        Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: digest('1'),
            root_digest: digest('2'),
            resolved_selector: selector.to_owned(),
            bytes: Vec::new(),
        })
    })
    .unwrap_or_else(|_| panic!("resident exact projection probe"))
    .expect("all exact projections are resident");
    assert_eq!(resident_reads, request.selectors.len());
    assert_eq!(resident.len(), request.selectors.len());
    let mut handoff = Some(resident);
    for selector in &request.selectors {
        let read = read_query_projection_handoff(
            &mut handoff,
            agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
            selector,
            |_projection, _selector| {
                resident_reads += 1;
                Err("resident handoff unexpectedly repeated the selector read".to_owned())
            },
        )
        .expect("ordered resident projection handoff");
        assert!(matches!(
            read,
            WorkspaceRuntimeSelectorRead::Projection {
                resolved_selector,
                ..
            } if resolved_selector == *selector
        ));
    }
    assert_eq!(resident_reads, request.selectors.len());
    assert!(handoff.as_ref().is_some_and(|reads| reads.is_empty()));

    let missing = request.selectors[1].clone();
    assert!(
        resident_exact_query_projections(&request, |_projection, selector| {
            if selector == missing {
                return Ok(WorkspaceRuntimeSelectorRead::ProjectionMissing {
                    generation_digest: digest('1'),
                    root_digest: digest('2'),
                    resolved_selector: selector.to_owned(),
                });
            }
            Ok(WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('1'),
                root_digest: digest('2'),
                resolved_selector: selector.to_owned(),
                bytes: Vec::new(),
            })
        })
        .unwrap_or_else(|_| panic!("missing projection routes to owner materialization"))
        .is_none()
    );
}

#[test]
fn process_cold_replay_accepts_only_the_requested_owner_selector() {
    use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

    let selector = &params().selectors[0];
    let direct = WorkspaceRuntimeSelectorRead::Projection {
        generation_digest: digest('1'),
        root_digest: digest('2'),
        resolved_selector: selector.clone(),
        bytes: b"direct".to_vec(),
    };
    assert!(durable_projection_is_direct(selector, &direct));

    let relocated = WorkspaceRuntimeSelectorRead::Projection {
        generation_digest: digest('1'),
        root_digest: digest('2'),
        resolved_selector: selector.replace("publication.org", "relocated.org"),
        bytes: b"relocated".to_vec(),
    };
    assert!(!durable_projection_is_direct(selector, &relocated));
    assert!(!durable_projection_is_direct(
        selector,
        &WorkspaceRuntimeSelectorRead::ProjectionMissing {
            generation_digest: digest('1'),
            root_digest: digest('2'),
            resolved_selector: selector.clone(),
        }
    ));
}

#[test]
fn process_cold_replay_rejects_a_stale_runtime_bundle() {
    let publication = execution_publication();
    assert!(durable_exact_execution_matches_current(
        &publication,
        &publication.workspace_identity,
        publication.runtime_bundle_digest.as_str(),
        publication.generation_digest.as_str(),
        publication.source_root_digest.as_str(),
    ));
    assert!(durable_exact_execution_matches_current(
        &publication,
        &publication.workspace_identity,
        publication.runtime_bundle_digest.as_str(),
        publication.generation_digest.as_str(),
        publication
            .source_root_digest
            .as_str()
            .strip_prefix("blake3-256:")
            .expect("raw exact-segment root digest"),
    ));
    assert!(!durable_exact_execution_matches_current(
        &publication,
        &publication.workspace_identity,
        &digest('0'),
        publication.generation_digest.as_str(),
        publication.source_root_digest.as_str(),
    ));
    assert!(!durable_exact_execution_matches_current(
        &publication,
        &publication.workspace_identity,
        publication.runtime_bundle_digest.as_str(),
        publication.generation_digest.as_str(),
        "not-a-content-digest",
    ));
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
        &digest('1'),
        &"2".repeat(64),
        &binding.project_workspace,
        &[
            ("org".into(), "asp-org".into()),
            ("rust".into(), "asp-rust".into()),
        ],
        Some((&trace, &bus.sender)),
        |_projection, selector| {
            Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: digest('1'),
                root_digest: "2".repeat(64),
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

#[test]
fn query_playbook_selector_failure_exposes_no_partial_materialization() {
    let binding = runtime_binding();
    let (reason_kind, receipt) = failed_query_receipt(materialize_query_playbook_receipt(
        "request-query-playbook-failed",
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
            if selector.starts_with("org://") {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('1'),
                    root_digest: "2".repeat(64),
                    resolved_selector: selector.to_owned(),
                    bytes: b"publication".to_vec(),
                })
            } else {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::OwnerMissing {
                    generation_digest: digest('1'),
                    root_digest: "2".repeat(64),
                })
            }
        },
    ));
    assert_eq!(reason_kind, "query-playbook-selector-not-materialized");
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
    let (reason_kind, receipt) = failed_query_receipt(materialize_query_playbook_receipt(
        "request-query-playbook-read-failed",
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
            if selector.starts_with("org://") {
                Ok(agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: digest('1'),
                    root_digest: "2".repeat(64),
                    resolved_selector: selector.to_owned(),
                    bytes: b"must-not-escape".to_vec(),
                })
            } else {
                Err("resident selector reader failed".to_owned())
            }
        },
    ));
    assert_eq!(reason_kind, "query-playbook-selector-read-failed");
    assert_eq!(receipt["terminal"]["state"], "failed");
    assert_eq!(receipt["terminal"]["terminalCount"], 1);
    assert_eq!(
        receipt["terminal"]["reasonKind"],
        "query-playbook-selector-read-failed"
    );
    assert_eq!(receipt["materializations"], serde_json::json!([]));
}
