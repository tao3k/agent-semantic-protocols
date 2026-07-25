use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::{
    context_run_mvcc::TursoMvccContextRunStore,
    turso_mvcc_store::{TursoMvccStore, TursoMvccStoreConfig},
};
use agent_semantic_context_product::{
    ActiveProgram, CONTEXT_PRODUCT_CANONICALIZATION_PROFILE, CONTEXT_PRODUCT_SCHEMA_ID,
    CONTEXT_PRODUCT_SCHEMA_VERSION, ClaimClass, ClosureDisposition, ContextBinding,
    DecisionRequirement, Digest, ExecutionAuthority, FrontierAntichain, Obligation,
    ObligationDisposition, ProofReuse, ProtocolId, SearchBudget, UncheckedContextProductStateV1,
};
use agent_semantic_loop::RunCommitStore;

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
                max_commands: 4,
                max_elapsed_ms: 2_000,
                max_packet_bytes: 16_384,
            },
        },
        execution: ExecutionAuthority::None,
        closure: ClosureDisposition::Open {
            open_obligation_ids: vec![obligation_id],
        },
        state_digest: digest("unset-state"),
    };
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();
    state
}

#[tokio::test]
async fn context_run_initialization_round_trips_through_mvcc_projection() {
    let mvcc = TursoMvccStore::open(TursoMvccStoreConfig::new(temp_database("round-trip")))
        .await
        .expect("open MVCC store");
    let store = TursoMvccContextRunStore::new(mvcc, id("client-db-authority"));
    let state = initial_state();

    let receipt = store
        .initialize(state.clone(), 1)
        .await
        .expect("initialize context run");
    receipt
        .authority_receipt
        .validate_for_state(&state)
        .expect("authority receipt");
    let loaded = store.load(&state.run_id).await.expect("load context run");
    assert_eq!(loaded.state, state);
    assert_eq!(loaded.authority_receipt, receipt.authority_receipt);
}
