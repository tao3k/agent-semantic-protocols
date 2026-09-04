use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_client_db::context_run_mvcc::TursoMvccContextRunStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig;
use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::CONTEXT_PRODUCT_CANONICALIZATION_PROFILE;
use agent_semantic_context_product::CONTEXT_PRODUCT_SCHEMA_ID;
use agent_semantic_context_product::CONTEXT_PRODUCT_SCHEMA_VERSION;
use agent_semantic_context_product::ClaimClass;
use agent_semantic_context_product::ClosureDisposition;
use agent_semantic_context_product::ContextBinding;
use agent_semantic_context_product::DecisionRequirement;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::EvidenceReceipt;
use agent_semantic_context_product::FrontierAntichain;
use agent_semantic_context_product::Obligation;
use agent_semantic_context_product::ObligationDisposition;
use agent_semantic_context_product::ProofReuse;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::RouteProposal;
use agent_semantic_context_product::SearchBudget;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_loop::AdmitRouteProgramRequest;
use agent_semantic_loop::GraphRouter;
use agent_semantic_loop::PortFuture;
use agent_semantic_loop::ProofResolver;
use agent_semantic_loop::RunCommitStore;
use agent_semantic_loop::TrustedClock;
use agent_semantic_loop::search_capability::AdvanceCapabilitySubject;
use agent_semantic_loop::search_capability::SearchLoopCapabilityCommand;
use agent_semantic_loop::search_capability::SearchLoopCapabilitySpend;
use agent_semantic_loop::search_capability::SearchLoopCapabilityStateBinding;
use agent_semantic_loop::search_capability::SearchLoopCapabilityStatus;
use agent_semantic_loop::search_capability::SearchLoopCapabilitySubject;
use agent_semantic_loop::search_capability::SearchLoopCapabilityToken;
use agent_semantic_loop::search_capability::SearchLoopCapabilityV1;
use agent_semantic_loop::search_capability::SearchLoopCapabilityValidationError;

fn temp_database(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-context-run-mvcc-{name}-{nanos}.turso"))
}

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("protocol id")
}

fn digest(value: &str) -> Digest {
    Digest::from_bytes(value.as_bytes())
}

fn initial_state() -> UncheckedContextProductStateV1 {
    let mut context = ContextBinding {
        workspace_id: id("workspace-1"),
        workspace_root_digest: digest("workspace-root"),
        base_source_digest: digest("base-source"),
        dirty_overlay_digest: digest("dirty-overlay"),
        source_snapshot_digest: digest("source-snapshot"),
        provider_digest: digest("provider"),
        parser_digest: digest("parser"),
        query_pack_digest: digest("query-pack"),
        fact_schema_digest: digest("fact-schema"),
        projection_schema_digest: digest("projection-schema"),
        policy_digest: digest("policy"),
        graph_revision_digest: digest("graph-revision"),
        context_epoch: 1,
        binding_digest: digest("unset-binding"),
    };
    context.binding_digest = context.recompute_binding_digest();
    let obligation_id = id("obligation-1");
    let mut frontier = FrontierAntichain {
        frontier_id: id("frontier-1"),
        graph_digest: digest("graph"),
        context_binding_digest: context.binding_digest.clone(),
        nodes: vec![],
        frontier_digest: digest("unset-frontier"),
    };
    frontier.frontier_digest = frontier.recompute_frontier_digest();
    let mut state = UncheckedContextProductStateV1 {
        schema_id: CONTEXT_PRODUCT_SCHEMA_ID.to_string(),
        schema_version: CONTEXT_PRODUCT_SCHEMA_VERSION,
        run_id: id("run-1"),
        revision: 0,
        previous_state_digest: None,
        last_event_sequence: 0,
        event_log_digest: digest("empty-event-log"),
        active_program: ActiveProgram::None,
        spent_action_keys: vec![],
        spent_action_ledger_digest: digest("unset-action-ledger"),
        authority_receipt_ref: id("authority-receipt-0"),
        canonicalization_profile: CONTEXT_PRODUCT_CANONICALIZATION_PROFILE.to_string(),
        context,
        obligations: vec![Obligation {
            obligation_id: obligation_id.clone(),
            claim_class: ClaimClass::Identity,
            claim_digest: digest("claim"),
            required: true,
            depends_on: vec![],
            disposition: ObligationDisposition::Open {
                reason_code: id("evidence-required"),
            },
        }],
        proof_reuse: ProofReuse::None {
            retained: vec![],
            invalidated_proof_refs: vec![],
        },
        frontier,
        decision: DecisionRequirement::Search {
            open_obligation_ids: vec![obligation_id.clone()],
            evidence_plan_ref: id("evidence-plan-1"),
            budget: SearchBudget {
                max_commands: 4.into(),
                max_elapsed_ms: 2_000.into(),
                max_packet_bytes: 16_384.into(),
                max_choice_depth: 4.into(),
                max_parallel: 2.into(),
                max_aggregate_provider_latency_ms: 2_000.into(),
                max_parent_visible_bytes: 16_384.into(),
            },
        },
        executions: vec![],
        joined_execution_groups: vec![],
        closure: ClosureDisposition::Open {
            open_obligation_ids: vec![obligation_id],
        },
        state_digest: digest("unset-state"),
    };
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();
    state
}

fn initial_search_loop_runtime(
    state: &UncheckedContextProductStateV1,
) -> agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1 {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/search-interactive-loop/runtime-active.v1.json"
    ))
    .expect("runtime fixture");
    value["runId"] = serde_json::json!(state.run_id.as_str());
    value["contextBindingDigest"] = serde_json::json!(state.context.binding_digest.as_str());
    value["openedAtRevision"] = serde_json::json!(0);
    value["activePanel"]["basedOnRevision"] = serde_json::json!(0);
    value["batches"] = serde_json::json!([]);
    let unchecked = serde_json::from_value(value).expect("unchecked runtime fixture");
    agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked)
        .expect("valid initial runtime")
}

#[tokio::test]
async fn context_run_initialization_round_trips_through_mvcc_projection() {
    let mvcc = TursoMvccStore::open(TursoMvccStoreConfig::new(temp_database("round-trip")))
        .await
        .expect("open MVCC store");
    let store = TursoMvccContextRunStore::new(mvcc, id("client-db-authority"));
    let state = initial_state();
    let runtime = initial_search_loop_runtime(&state);

    let receipt = store
        .initialize(state.clone(), Vec::new(), Some(runtime.clone()), 1)
        .await
        .expect("initialize context run");
    receipt
        .authority_receipt
        .validate_for_state(&state)
        .expect("authority receipt");
    let loaded = store.load(&state.run_id).await.expect("load context run");
    assert_eq!(loaded.state, state);
    assert_eq!(loaded.authority_receipt, receipt.authority_receipt);
    let located = store
        .load_search_loop_runtime(runtime.loop_id())
        .await
        .expect("resolve search-loop alias")
        .expect("runtime partition");
    let located_runtime =
        agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(
            located
                .search_loop_runtime
                .expect("runtime binding in projection"),
        )
        .expect("valid located runtime");
    assert_eq!(located_runtime, runtime);
    assert_eq!(
        located_runtime
            .active_panel()
            .expect("active panel")
            .graph_cursor_artifact()
            .artifact_schema_id()
            .as_str(),
        "agent.semantic-protocols.search-graph-cursor"
    );
    let router = GraphRouter::new(store, UnusedProofResolver, FixedClock(2));
    let routed = router
        .load_search_loop(runtime.loop_id())
        .await
        .expect("router resolves loop id through typed runtime port");
    assert_eq!(routed.run_id(), &state.run_id);
}

#[tokio::test]
async fn search_loop_alias_conflict_rolls_back_second_partition() {
    let mvcc = TursoMvccStore::open(TursoMvccStoreConfig::new(temp_database("alias-conflict")))
        .await
        .expect("open MVCC store");
    let store = TursoMvccContextRunStore::new(mvcc, id("client-db-authority"));
    let first_state = initial_state();
    let first_runtime = initial_search_loop_runtime(&first_state);
    store
        .initialize(
            first_state.clone(),
            Vec::new(),
            Some(first_runtime.clone()),
            1,
        )
        .await
        .expect("initialize first loop");

    let mut conflicting_state = initial_state();
    conflicting_state.run_id = id("run-search-conflict");
    conflicting_state.authority_receipt_ref = id("receipt-authority-conflict");
    conflicting_state.state_digest = conflicting_state.recompute_state_digest();
    conflicting_state
        .validate()
        .expect("conflicting initial state remains valid");
    let conflicting_runtime = initial_search_loop_runtime(&conflicting_state);
    assert_eq!(conflicting_runtime.loop_id(), first_runtime.loop_id());

    store
        .initialize(
            conflicting_state.clone(),
            Vec::new(),
            Some(conflicting_runtime),
            2,
        )
        .await
        .expect_err("one loop id must not alias two context partitions");
    assert!(
        store.load(&conflicting_state.run_id).await.is_err(),
        "failed alias initialization must roll back the second partition"
    );
    let located = store
        .load_search_loop_runtime(first_runtime.loop_id())
        .await
        .expect("resolve original alias")
        .expect("original loop remains present");
    assert_eq!(located.state.run_id, first_state.run_id);
}

#[tokio::test]
async fn route_admission_atomically_consumes_capability_and_replays_receipt_after_reopen() {
    let database_path = temp_database("capability-consumption");
    let mvcc = TursoMvccStore::open(TursoMvccStoreConfig::new(&database_path))
        .await
        .expect("open MVCC store");
    let store = TursoMvccContextRunStore::new(mvcc.clone(), id("client-db-authority"));
    let state = initial_state();
    let raw_token = "capability.9999999999999999999999999999999999999999999999999999999999999999";
    let token = SearchLoopCapabilityToken::parse(raw_token).expect("parse raw capability token");
    let resident_identity_ref = id("resident-asp-explore");
    let loop_id = id("loop-search-1");
    let capability = SearchLoopCapabilityV1::issue(
        id("capability-advance-1"),
        token.digest(),
        resident_identity_ref.clone(),
        loop_id.clone(),
        SearchLoopCapabilityStateBinding::new(
            state.run_id.clone(),
            state.revision,
            state.state_digest.clone(),
            state.authority_receipt_ref.clone(),
        ),
        state.context.binding_digest.clone(),
        SearchLoopCapabilitySubject::Advance(AdvanceCapabilitySubject::new(
            id("panel-search-1"),
            id("choice-serial-1"),
            id("proposal-serial-1"),
            digest("proposal-serial-1"),
            id("group-serial-1"),
        )),
        1,
        60_001,
    )
    .expect("issue search-loop capability");

    store
        .initialize(state.clone(), vec![capability.clone()], None, 1)
        .await
        .expect("initialize context run with capability");
    let authorized_capability = store
        .load_search_loop_capability(&state.run_id, &token.digest())
        .await
        .expect("load capability")
        .expect("capability must exist");
    let router = GraphRouter::new(store, UnusedProofResolver, FixedClock(2));
    let current = router
        .load(&state.run_id)
        .await
        .expect("load authoritative context run");
    authorized_capability
        .authorize(
            SearchLoopCapabilityCommand::Advance,
            &resident_identity_ref,
            &loop_id,
            &current,
            2,
        )
        .expect("raw token digest must authorize the issued advance capability");

    let (proposal, program) = serial_route(&state);
    let committed = router
        .admit_route_program_with_capability_spend(
            AdmitRouteProgramRequest {
                run_id: state.run_id.clone(),
                expected_revision: state.revision,
                expected_state_digest: state.state_digest.clone(),
                expected_context_binding_digest: state.context.binding_digest.clone(),
                event_id: id("event-route-program-admitted-1"),
                proposal,
                program,
            },
            SearchLoopCapabilitySpend::new(capability.capability_id().clone(), token.digest()),
        )
        .await
        .expect("admit route program and consume capability");

    let partition_key = format!("context-run:{}", state.run_id.as_str());
    let head = mvcc
        .load_partition_head(&partition_key)
        .await
        .expect("load committed partition head")
        .expect("partition head must exist");
    let projection: serde_json::Value =
        serde_json::from_slice(&head.projection).expect("decode committed projection");
    let persisted_capability = &projection["searchLoopCapabilities"][0];
    assert_eq!(head.revision, committed.revision());
    assert_eq!(
        persisted_capability["consumedAtRevision"],
        serde_json::json!(committed.revision())
    );
    assert_eq!(
        persisted_capability["consumptionReceiptRef"],
        serde_json::json!(committed.authority_receipt().receipt_id.as_str())
    );
    assert!(
        !contains_bytes(&head.projection, raw_token.as_bytes()),
        "raw capability token must not be persisted in the authoritative projection"
    );
    let records = mvcc
        .read_partition_records(&partition_key)
        .await
        .expect("read committed route event");
    assert_eq!(records.len(), 1);
    assert!(
        records
            .iter()
            .all(|record| !contains_bytes(record.payload(), raw_token.as_bytes())),
        "raw capability token must not be persisted in append records"
    );

    let expected_receipt_ref = committed.authority_receipt().receipt_id.clone();
    drop(router);
    drop(mvcc);

    let reopened_mvcc = TursoMvccStore::open(TursoMvccStoreConfig::new(&database_path))
        .await
        .expect("reopen MVCC store");
    let reopened_store = TursoMvccContextRunStore::new(reopened_mvcc, id("client-db-authority"));
    let recovered_capability = reopened_store
        .load_search_loop_capability(&state.run_id, &token.digest())
        .await
        .expect("reload capability after reopen")
        .expect("consumed capability must remain queryable by token digest");
    assert_eq!(
        recovered_capability.status(),
        SearchLoopCapabilityStatus::Consumed
    );
    assert_eq!(
        recovered_capability.consumed_at_revision(),
        Some(committed.revision())
    );
    assert_eq!(
        recovered_capability.consumption_receipt_ref(),
        Some(&expected_receipt_ref)
    );

    let replay_router = GraphRouter::new(reopened_store, UnusedProofResolver, FixedClock(3));
    let recovered_state = replay_router
        .load(&state.run_id)
        .await
        .expect("reload authoritative state after reopen");
    assert_eq!(
        recovered_capability.authorize(
            SearchLoopCapabilityCommand::Advance,
            &resident_identity_ref,
            &loop_id,
            &recovered_state,
            3,
        ),
        Err(SearchLoopCapabilityValidationError::Status),
        "one-shot capability replay must be rejected without losing its consumption receipt"
    );
}

fn serial_route(state: &UncheckedContextProductStateV1) -> (RouteProposal, RouteProgram) {
    let catalog_digest = digest("catalog-1");
    let evidence_predicate = serde_json::json!({
        "predicateId": "predicate-1",
        "claimClass": "identity",
        "scopeDigest": digest("scope-1"),
        "acceptedSchemaIds": ["semantic-search-packet.v1"],
        "requiredFields": ["canonicalItemSelector"],
        "completeness": "complete-scope",
        "requiresFreshBinding": true,
        "requiresUntruncated": true,
        "maxResults": 1
    });
    let mut proposal: RouteProposal = serde_json::from_value(serde_json::json!({
        "proposalId": "proposal-serial-1",
        "runId": state.run_id.as_str(),
        "basedOnRevision": state.revision,
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "intentDigest": digest("intent-1"),
        "catalogDigest": catalog_digest,
        "proposedBy": "graph-turbo",
        "agentReasoningReceiptRef": "reasoning-serial-1",
        "graphDerivationReceiptRef": "derivation-serial-1",
        "obligationIds": ["obligation-1"],
        "nodes": [{
            "nodeId": "node-1",
            "actionClass": "provider-search",
            "providerId": "rust-provider",
            "languageId": "rust",
            "catalogId": "resolve-symbol.v1",
            "inputTemplateDigest": digest("input-template-1"),
            "coversObligationIds": ["obligation-1"],
            "dependsOnNodeIds": [],
            "requiredClosure": "discovery",
            "evidencePredicate": evidence_predicate
        }],
        "edges": [],
        "executionGroups": [{
            "groupId": "group-serial-1",
            "mode": "serial",
            "nodeIds": ["node-1"],
            "joinPolicy": "all-required",
            "maxParallel": 1,
            "derivationReceiptRef": "derivation-group-serial-1"
        }],
        "joins": [],
        "estimateReceiptRefs": [],
        "budgetProposal": route_budget(),
        "proposalDigest": digest("pending-proposal")
    }))
    .expect("deserialize serial route proposal");
    proposal.proposal_digest = proposal.recompute_proposal_digest();

    let mut program: RouteProgram = serde_json::from_value(serde_json::json!({
        "programId": "program-serial-1",
        "proposalId": proposal.proposal_id.as_str(),
        "runId": state.run_id.as_str(),
        "admittedAtRevision": state.revision,
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "graphDigest": digest("pending-graph"),
        "catalogDigest": proposal.catalog_digest.as_str(),
        "stages": [{
            "stageId": "stage-1",
            "proposalNodeId": "node-1",
            "providerId": "rust-provider",
            "catalogId": "resolve-symbol.v1",
            "inputTemplateDigest": digest("input-template-1"),
            "coversObligationIds": ["obligation-1"],
            "requiredClosure": "discovery",
            "evidencePredicate": evidence_predicate
        }],
        "executionGroups": [{
            "groupId": "group-serial-1",
            "mode": "serial",
            "stageIds": ["stage-1"],
            "joinPolicy": "all-required",
            "maxParallel": 1,
            "derivationReceiptRef": "derivation-group-serial-1"
        }],
        "edges": [],
        "joins": [],
        "budgetLimit": route_budget(),
        "programDigest": digest("pending-program")
    }))
    .expect("deserialize serial route program");
    program.graph_digest = program.recompute_graph_digest();
    program.program_digest = program.recompute_program_digest();
    (proposal, program)
}

fn route_budget() -> serde_json::Value {
    serde_json::json!({
        "maxCommands": 1,
        "maxElapsedMs": 1_000,
        "maxPacketBytes": 10_000,
        "maxChoiceDepth": 1,
        "maxParallel": 1,
        "maxAggregateProviderLatencyMs": 1_000,
        "maxParentVisibleBytes": 2_000
    })
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

struct UnusedProofResolver;

impl ProofResolver for UnusedProofResolver {
    type Error = String;

    fn resolve<'a>(
        &'a self,
        _proof_ref: &'a ProtocolId,
    ) -> PortFuture<'a, EvidenceReceipt, Self::Error> {
        Box::pin(async { Err("proof resolver is unused during route admission".to_owned()) })
    }
}

#[derive(Clone, Copy)]
struct FixedClock(u64);

impl TrustedClock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}
