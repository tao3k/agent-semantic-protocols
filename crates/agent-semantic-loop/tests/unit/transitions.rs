use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use agent_semantic_context_product::{
    ActiveProgram, ClosureDisposition, ClosureProof, Digest, EffectClass, EvidenceReceipt,
    ExecutionAuthority, ProtocolId, RouteProgram, StateAuthorityReceipt,
    UncheckedContextProductStateV1,
};

use crate::ports::{
    AuthoritativeStateRecord, CompareAndAppendOutcome, PortFuture, RunCommit, RunCommitReceipt,
};
use crate::{
    FinalizeClosureRequest, GraphRouter, GraphRouterError, ProofResolver, RunCommitStore,
    TrustedClock,
};

struct MemoryStore {
    record: Mutex<AuthoritativeStateRecord>,
}

impl RunCommitStore for MemoryStore {
    type Error = String;

    fn load<'a>(
        &'a self,
        _run_id: &'a ProtocolId,
    ) -> PortFuture<'a, AuthoritativeStateRecord, Self::Error> {
        Box::pin(async move {
            self.record
                .lock()
                .map_err(|error| error.to_string())
                .map(|record| record.clone())
        })
    }

    fn compare_and_append<'a>(
        &'a self,
        commit: RunCommit,
    ) -> PortFuture<'a, CompareAndAppendOutcome, Self::Error> {
        Box::pin(async move {
            let next_state = commit.next_state().clone();
            let mut authority_receipt = StateAuthorityReceipt {
                receipt_id: next_state.authority_receipt_ref.clone(),
                authority_id: id("test-authority"),
                run_id: next_state.run_id.clone(),
                revision: next_state.revision,
                state_digest: next_state.state_digest.clone(),
                event_log_digest: next_state.event_log_digest.clone(),
                issued_at_ms: commit.committed_at_ms(),
                receipt_digest: Digest::from_bytes(b"pending-authority-receipt"),
            };
            authority_receipt.receipt_digest = authority_receipt.recompute_receipt_digest();
            let next_record = AuthoritativeStateRecord {
                state: next_state,
                authority_receipt: authority_receipt.clone(),
            };
            *self.record.lock().map_err(|error| error.to_string())? = next_record;
            Ok(CompareAndAppendOutcome::Committed(RunCommitReceipt {
                authority_receipt,
            }))
        })
    }
}

#[derive(Clone)]
struct StaticProofResolver {
    receipt: EvidenceReceipt,
}

impl ProofResolver for StaticProofResolver {
    type Error = String;

    fn resolve<'a>(
        &'a self,
        _proof_ref: &'a ProtocolId,
    ) -> PortFuture<'a, EvidenceReceipt, Self::Error> {
        Box::pin(async move { Ok(self.receipt.clone()) })
    }
}

#[derive(Clone, Copy)]
struct FixedClock(u64);

impl TrustedClock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

#[test]
fn closure_rejects_active_execution_authority() {
    let record = authoritative_record(granted_execution());
    let request = closure_request(&record);
    let router = router(record);

    let error =
        block_on(router.finalize_closure(request)).expect_err("active grant must block closure");
    assert!(matches!(
        error,
        GraphRouterError::InvalidTransition(
            "search closure requires no active execution authority"
        )
    ));
}

#[test]
fn closure_rejects_missing_required_proof() {
    let record = authoritative_record(ExecutionAuthority::None);
    let mut request = closure_request(&record);
    request.closed_obligations.clear();
    let router = router(record);

    let error = block_on(router.finalize_closure(request))
        .expect_err("every required obligation must have one proof");
    assert!(matches!(
        error,
        GraphRouterError::InvalidTransition(
            "closure proofs must cover every required obligation exactly once"
        )
    ));
}

#[test]
fn closure_accepts_terminal_execution_and_commits_verified_proof() {
    let record = authoritative_record(consumed_execution());
    let request = closure_request(&record);
    let router = router(record);

    let committed = block_on(router.finalize_closure(request))
        .expect("terminal execution and matching proof must close");
    assert!(matches!(
        committed.wire().closure,
        ClosureDisposition::Finalized { .. }
    ));
    assert!(matches!(
        committed.wire().execution,
        ExecutionAuthority::Consumed { .. }
    ));
}

fn router(
    record: AuthoritativeStateRecord,
) -> GraphRouter<MemoryStore, StaticProofResolver, FixedClock> {
    let receipt = evidence_receipt(&record.state);
    GraphRouter::new(
        MemoryStore {
            record: Mutex::new(record),
        },
        StaticProofResolver { receipt },
        FixedClock(100),
    )
}

fn closure_request(record: &AuthoritativeStateRecord) -> FinalizeClosureRequest {
    let state = &record.state;
    FinalizeClosureRequest {
        run_id: state.run_id.clone(),
        expected_revision: state.revision,
        expected_state_digest: state.state_digest.clone(),
        expected_context_binding_digest: state.context.binding_digest.clone(),
        closed_obligations: vec![closure_proof()],
    }
}

fn closure_proof() -> ClosureProof {
    serde_json::from_value(serde_json::json!({
        "obligationId": "obligation-1",
        "claimClass": "identity",
        "verdict": "supported",
        "proofRefs": ["proof-1"],
        "supportPaths": ["src/lib.rs"],
        "evidenceScope": {
            "scopeDigest": digest("scope"),
            "complete": true,
            "truncated": false,
            "snapshotContinuous": true,
            "providerAdmitted": true
        }
    }))
    .expect("closure proof must deserialize")
}

fn evidence_receipt(state: &UncheckedContextProductStateV1) -> EvidenceReceipt {
    let proof = closure_proof();
    serde_json::from_value(serde_json::json!({
        "proofRef": "proof-1",
        "claimClass": "identity",
        "claimDigest": state.obligations[0].claim_digest.as_str(),
        "verdict": "supported",
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "sourceSnapshotDigest": state.context.source_snapshot_digest.as_str(),
        "providerDigest": state.context.provider_digest.as_str(),
        "parserDigest": state.context.parser_digest.as_str(),
        "queryPackDigest": state.context.query_pack_digest.as_str(),
        "scopeDigest": proof.evidence_scope.scope_digest.as_str(),
        "supportClosureDigest": super::canonical_digest(&proof.support_paths).as_str(),
        "freshnessReceiptRef": "freshness-1",
        "packetDigest": digest("packet")
    }))
    .expect("evidence receipt must deserialize")
}

fn authoritative_record(execution: ExecutionAuthority) -> AuthoritativeStateRecord {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/context-product-state.v1.fixtures.json"
    ))
    .expect("fixtures must parse");
    let mut state: UncheckedContextProductStateV1 = serde_json::from_value(
        fixtures["fixtures"]
            .as_array()
            .expect("fixtures array")
            .iter()
            .find(|fixture| fixture["name"] == "valid-open-search-state")
            .expect("open state fixture")["value"]
            .clone(),
    )
    .expect("open state must deserialize");

    state.revision = 3;
    state.last_event_sequence = 3;
    state.previous_state_digest = Some(Digest::from_bytes(b"previous-state"));
    state.context.binding_digest = state.context.recompute_binding_digest();
    let mut program: RouteProgram = serde_json::from_value(serde_json::json!({
        "programId": "program-1",
        "proposalId": "proposal-1",
        "runId": state.run_id.as_str(),
        "admittedAtRevision": 1,
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "graphDigest": digest("pending-graph"),
        "stages": [{
            "stageId": "stage-1",
            "proposalNodeId": "node-1",
            "providerId": "provider-1",
            "operation": "search",
            "inputTemplateDigest": digest("input-template"),
            "coversObligationIds": ["obligation-1"],
            "evidencePredicate": {
                "predicateId": "predicate-1",
                "claimClass": "identity",
                "scopeDigest": digest("scope"),
                "acceptedSchemaIds": ["semantic-search-packet.v1"],
                "requiredFields": ["symbol"],
                "completeness": "complete-scope",
                "requiresFreshBinding": true,
                "requiresUntruncated": true,
                "maxResults": 8
            }
        }],
        "edges": [],
        "joins": [],
        "budgetLimit": {
            "maxCommands": 4,
            "maxElapsedMs": 10000,
            "maxPacketBytes": 65536
        },
        "programDigest": digest("pending-program")
    }))
    .expect("route program must deserialize");
    program.graph_digest = program.recompute_graph_digest();
    program.program_digest = program.recompute_program_digest();
    state.active_program = ActiveProgram::Admitted {
        proposal_id: program.proposal_id.clone(),
        proposal_digest: Digest::from_bytes(b"proposal"),
        intent_digest: Digest::from_bytes(b"intent"),
        program_id: program.program_id.clone(),
        program_digest: program.program_digest.clone(),
        program: Box::new(program.clone()),
        graph_digest: program.graph_digest.clone(),
        admitted_at_revision: 1,
        context_binding_digest: state.context.binding_digest.clone(),
    };
    state.frontier.graph_digest = program.graph_digest;
    state.frontier.context_binding_digest = state.context.binding_digest.clone();
    state.frontier.frontier_digest = state.frontier.recompute_frontier_digest();
    state.execution = execution;
    state.closure = ClosureDisposition::Open {
        open_obligation_ids: vec![id("obligation-1")],
    };
    state.authority_receipt_ref = id("authority-receipt-3");
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();
    state.validate().expect("test state must validate");

    let mut authority_receipt = StateAuthorityReceipt {
        receipt_id: state.authority_receipt_ref.clone(),
        authority_id: id("test-authority"),
        run_id: state.run_id.clone(),
        revision: state.revision,
        state_digest: state.state_digest.clone(),
        event_log_digest: state.event_log_digest.clone(),
        issued_at_ms: 99,
        receipt_digest: Digest::from_bytes(b"pending-authority-receipt"),
    };
    authority_receipt.receipt_digest = authority_receipt.recompute_receipt_digest();
    AuthoritativeStateRecord {
        state,
        authority_receipt,
    }
}

fn granted_execution() -> ExecutionAuthority {
    serde_json::from_value(serde_json::json!({
        "kind": "granted",
        "admissionId": "admission-1",
        "admissionDigest": digest("admission"),
        "grantId": "grant-1",
        "grantDigest": digest("grant"),
        "actionKey": digest("action"),
        "programDigest": digest("program"),
        "stageId": "stage-1",
        "providerId": "provider-1",
        "operation": "search",
        "resolvedInputDigest": digest("input"),
        "policyDigest": digest("policy"),
        "dependencyDigest": digest("dependency"),
        "budgetReservationId": "budget-1",
        "budgetChargeKey": digest("charge"),
        "effectClass": effect_class(),
        "leaseFence": 1,
        "expiresAtMs": 1000,
        "providerIdempotencyKey": "idempotency-1"
    }))
    .expect("granted execution must deserialize")
}

fn consumed_execution() -> ExecutionAuthority {
    serde_json::from_value(serde_json::json!({
        "kind": "consumed",
        "admissionId": "admission-1",
        "grantId": "grant-1",
        "grantDigest": digest("grant"),
        "actionKey": digest("action"),
        "attemptId": "attempt-1",
        "resultReceiptRef": "result-1",
        "resultDigest": digest("result"),
        "leaseFence": 1
    }))
    .expect("consumed execution must deserialize")
}

fn effect_class() -> serde_json::Value {
    serde_json::to_value(EffectClass::ReadOnly).expect("effect class must serialize")
}

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("valid protocol id")
}

fn digest(value: &str) -> String {
    Digest::from_bytes(value.as_bytes()).as_str().to_owned()
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
