use agent_semantic_protocol::context_product_state::{
    ActiveProgram, CONTEXT_PRODUCT_CANONICALIZATION_PROFILE, CONTEXT_PRODUCT_SCHEMA_ID,
    CONTEXT_PRODUCT_SCHEMA_VERSION, ClaimClass, ClosureDisposition, ContextBinding,
    DecisionRequirement, Digest, ExecutionAuthority, FrontierAntichain, FrontierNode, Obligation,
    ObligationDisposition, ParserOwnedCommandAdmission, ProofReuse, ProofReuseMode, ProtocolId,
    RecommendedNextCandidate, RetainedProof, SearchBudget, UncheckedContextProductStateV1,
    ValidationError,
};

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("test protocol id")
}

fn digest(value: &str) -> Digest {
    Digest::from_bytes(value.as_bytes())
}

fn open_state(claim_class: ClaimClass) -> UncheckedContextProductStateV1 {
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
        schema_id: CONTEXT_PRODUCT_SCHEMA_ID.to_owned(),
        schema_version: CONTEXT_PRODUCT_SCHEMA_VERSION,
        run_id: id("run-1"),
        revision: 1,
        previous_state_digest: Some(digest("previous-state")),
        last_event_sequence: 0,
        event_log_digest: digest("event-log"),
        active_program: ActiveProgram::None,
        spent_action_keys: vec![],
        spent_action_ledger_digest: digest("unset-spent-action-ledger"),
        authority_receipt_ref: id("authority-receipt-1"),
        canonicalization_profile: CONTEXT_PRODUCT_CANONICALIZATION_PROFILE.to_owned(),
        context,
        obligations: vec![Obligation {
            obligation_id: obligation_id.clone(),
            claim_class,
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

fn candidate(state: &UncheckedContextProductStateV1) -> RecommendedNextCandidate {
    RecommendedNextCandidate {
        language_id: id("rust"),
        operation: id("search.owner"),
        argv: vec!["asp".into(), "rust".into(), "search".into()],
        action_identity: digest("action-1"),
        context_binding_digest: state.context.binding_digest.clone(),
        covers_obligation_ids: vec![id("obligation-1")],
    }
}

#[test]
fn accepts_open_search_state() {
    open_state(ClaimClass::Identity)
        .validate()
        .expect("valid open search state");
}

#[test]
fn rejects_unknown_legacy_state_field() {
    let mut value = serde_json::to_value(open_state(ClaimClass::Identity)).expect("serialize");
    value
        .as_object_mut()
        .expect("state object")
        .insert("legacyProjectFact".into(), serde_json::json!(true));

    let error = serde_json::from_value::<UncheckedContextProductStateV1>(value)
        .expect_err("closed v1 object");
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn rejects_resolved_obligation_without_proof_refs() {
    let mut state = open_state(ClaimClass::Identity);
    state.obligations[0].disposition = ObligationDisposition::Resolved {
        verdict: agent_semantic_protocol::context_product_state::EvidenceVerdict::Supported,
        proof_refs: vec![],
    };
    state.frontier.nodes.clear();
    state.frontier.frontier_digest = state.frontier.recompute_frontier_digest();
    state.decision = DecisionRequirement::None;
    state.closure = ClosureDisposition::Finalized {
        receipt_id: id("receipt-1"),
        receipt_digest: digest("receipt-1"),
    };
    state.state_digest = state.recompute_state_digest();

    assert!(matches!(
        state.validate(),
        Err(ValidationError::EmptyRequiredCollection("proofRefs"))
    ));
}

#[test]
fn rejects_partial_reuse_without_invalidated_proof() {
    let mut state = open_state(ClaimClass::Identity);
    state.proof_reuse = ProofReuse::Partial {
        retained: vec![RetainedProof {
            proof_ref: id("proof-1"),
            mode: ProofReuseMode::Direct,
            context_binding_digest: state.context.binding_digest.clone(),
            rebase_certificate_ref: None,
        }],
        invalidated_proof_refs: vec![],
    };
    state.state_digest = state.recompute_state_digest();

    assert_eq!(
        state.validate(),
        Err(ValidationError::EmptyRequiredCollection(
            "invalidatedProofRefs"
        ))
    );
}

#[test]
fn rejects_frontier_containing_predecessor_and_successor() {
    let mut state = open_state(ClaimClass::Identity);
    let predecessor = id("frontier-node-1");
    state.frontier.nodes = vec![
        FrontierNode {
            node_id: predecessor.clone(),
            program_id: id("program-1"),
            stage_id: id("stage-1"),
            depth: 1,
            predecessor_ids: vec![],
            ancestor_node_ids: vec![],
            open_obligation_ids: vec![id("obligation-1")],
            proof_refs: vec![],
        },
        FrontierNode {
            node_id: id("frontier-node-2"),
            program_id: id("program-1"),
            stage_id: id("stage-2"),
            depth: 2,
            predecessor_ids: vec![predecessor],
            ancestor_node_ids: vec![id("frontier-node-1")],
            open_obligation_ids: vec![id("obligation-1")],
            proof_refs: vec![],
        },
    ];
    state.frontier.frontier_digest = state.frontier.recompute_frontier_digest();
    state.state_digest = state.recompute_state_digest();

    assert_eq!(state.validate(), Err(ValidationError::FrontierNotAntichain));
}

struct RejectParser;

impl ParserOwnedCommandAdmission for RejectParser {
    type Error = &'static str;

    fn validate(
        &self,
        _state: &UncheckedContextProductStateV1,
        _candidate: &RecommendedNextCandidate,
    ) -> Result<(), Self::Error> {
        Err("selector is not parser-admitted")
    }
}

#[test]
fn rejects_recommended_next_not_admitted_by_parser() {
    let state = open_state(ClaimClass::Identity);
    let error = RejectParser
        .validate(&state, &candidate(&state))
        .expect_err("parser owns command admission");
    assert_eq!(error, "selector is not parser-admitted");
}

#[test]
fn rejects_duplicate_recommended_next_action_identity() {
    let mut state = open_state(ClaimClass::Identity);
    let next = candidate(&state);
    state.spent_action_keys = vec![next.action_identity.clone(), next.action_identity.clone()];
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();

    assert_eq!(
        state.validate(),
        Err(ValidationError::DuplicateActionIdentity)
    );
}
