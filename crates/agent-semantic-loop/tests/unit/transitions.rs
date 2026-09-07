// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::ClosureDisposition;
use agent_semantic_context_product::ClosureProof;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::EffectClass;
use agent_semantic_context_product::EvidenceReceipt;
use agent_semantic_context_product::ExecutionAuthority;
use agent_semantic_context_product::JoinPolicy;
use agent_semantic_context_product::JoinedExecutionGroup;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::StateAuthorityReceipt;
use agent_semantic_context_product::UncheckedContextProductStateV1;

use crate::AdmitSearchLoopDirectiveRequest;
use crate::ExecutionDispatchAdmission;
use crate::FinalizeClosureRequest;
use crate::GraphRouter;
use crate::GraphRouterError;
use crate::JoinExecutionGroupRequest;
use crate::ProofResolver;
use crate::ProviderExecutionDispatch;
use crate::ProviderExecutionResult;
use crate::RunCommitStore;
use crate::SearchExecutionDriver;
use crate::SearchLoopAdvanceDispatch;
use crate::SearchLoopAdvanceRequest;
use crate::SearchLoopPollDispatch;
use crate::SearchLoopPollRequest;
use crate::TrustedClock;
use crate::ports::AuthoritativeStateRecord;
use crate::ports::CompareAndAppendOutcome;
use crate::ports::PortFuture;
use crate::ports::RunCommit;
use crate::ports::RunCommitReceipt;
use crate::search_loop::SearchLoopDirective;
use crate::search_loop::SearchLoopReducer;
use crate::search_loop::SearchLoopSnapshot;

struct MemoryStore {
    record: Mutex<AuthoritativeStateRecord>,
    commits: Arc<Mutex<Vec<RunCommit>>>,
}

type TestRouter = GraphRouter<MemoryStore, StaticProofResolver, FixedClock>;
type CommitLog = Arc<Mutex<Vec<RunCommit>>>;

#[test]
fn authoritative_load_preserves_typed_capability_validation_error() {
    let mut record = authoritative_record_for_program(Vec::new(), "serial", 1);
    record.search_loop_capabilities = vec![
        serde_json::from_str(include_str!(
            "../../../../schemas/fixtures/search-interactive-loop/invalid-poll-capability-consumed.v1.json"
        ))
        .expect("invalid capability fixture must deserialize"),
    ];
    let run_id = record.state.run_id.clone();
    let router = router(record);

    let error = block_on(router.load(&run_id))
        .expect_err("invalid persisted capability must reject authoritative state");
    assert_eq!(
        error,
        GraphRouterError::Capability(
            crate::search_capability::SearchLoopCapabilityValidationError::Status
        )
    );
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
            self.commits
                .lock()
                .map_err(|error| error.to_string())?
                .push(commit.clone());
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
                search_loop_capabilities: commit.search_loop_capabilities().to_vec(),
                search_loop_runtime: commit.search_loop_runtime().cloned(),
            };
            *self.record.lock().map_err(|error| error.to_string())? = next_record;
            Ok(CompareAndAppendOutcome::Committed(RunCommitReceipt {
                authority_receipt,
            }))
        })
    }
}

#[test]
fn batch_admission_is_one_dispatch_in_one_mvcc_revision() {
    let record = authoritative_record_for_program(Vec::new(), "batch", 2);
    let request = admit_directive_request(
        &record,
        SearchLoopDirective::AdmitBatch {
            group_id: id("group-1"),
            stage_ids: vec![id("stage-1"), id("stage-2")],
            batch_capability_ref: id("batch-capability-1"),
        },
        vec![ExecutionDispatchAdmission {
            event_id: id("event-batch-admitted"),
            stage_ids: vec![id("stage-1"), id("stage-2")],
            input_facts_digest: Digest::from_bytes(b"batch-inputs"),
        }],
    );
    let initial_revision = record.state.revision;
    let (router, commits) = router_with_commits(record);

    let committed = block_on(router.admit_search_loop_directive(request))
        .expect("batch group must admit as one provider dispatch");
    assert_eq!(committed.revision(), initial_revision + 1);
    assert_eq!(committed.executions().len(), 1);
    assert_eq!(
        committed.executions()[0].stage_ids(),
        [id("stage-1"), id("stage-2")]
    );
    let commits = commits.lock().expect("commit log");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].events().len(), 1);
}

#[test]
fn parallel_admission_fans_out_atomically_in_one_mvcc_revision() {
    let record = authoritative_record_for_program(Vec::new(), "parallel", 2);
    let request = admit_directive_request(
        &record,
        SearchLoopDirective::AdmitParallel {
            group_id: id("group-1"),
            stage_ids: vec![id("stage-1"), id("stage-2")],
            max_parallel: 2,
            independence_proof_ref: id("independence-proof-1"),
        },
        vec![
            ExecutionDispatchAdmission {
                event_id: id("event-parallel-admitted-1"),
                stage_ids: vec![id("stage-1")],
                input_facts_digest: Digest::from_bytes(b"parallel-input-1"),
            },
            ExecutionDispatchAdmission {
                event_id: id("event-parallel-admitted-2"),
                stage_ids: vec![id("stage-2")],
                input_facts_digest: Digest::from_bytes(b"parallel-input-2"),
            },
        ],
    );
    let initial_revision = record.state.revision;
    let initial_sequence = record.state.last_event_sequence;
    let (router, commits) = router_with_commits(record);

    let committed = block_on(router.admit_search_loop_directive(request))
        .expect("parallel group must admit all independent dispatches atomically");
    assert_eq!(committed.revision(), initial_revision + 1);
    assert_eq!(committed.wire().last_event_sequence, initial_sequence + 2);
    assert_eq!(committed.executions().len(), 2);
    assert!(committed.executions().iter().all(|execution| {
        execution.execution_group_id() == &id("group-1") && execution.stage_ids().len() == 1
    }));
    let commits = commits.lock().expect("commit log");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].events().len(), 2);
}

#[test]
fn consumed_group_must_persist_join_before_closure() {
    let record = authoritative_record_for_program(vec![consumed_execution()], "serial", 1);
    let request = JoinExecutionGroupRequest {
        run_id: record.state.run_id.clone(),
        expected_revision: record.state.revision,
        expected_state_digest: record.state.state_digest.clone(),
        expected_context_binding_digest: record.state.context.binding_digest.clone(),
        event_id: id("event-group-joined"),
        execution_group_id: id("group-1"),
    };
    let initial_revision = record.state.revision;
    let (router, commits) = router_with_commits(record);

    let committed =
        block_on(router.join_execution_group(request)).expect("consumed group must join");
    assert_eq!(committed.revision(), initial_revision + 1);
    assert_eq!(committed.wire().joined_execution_groups.len(), 1);
    assert_eq!(
        committed.wire().joined_execution_groups[0].result_receipt_refs,
        [id("result-1")]
    );
    let snapshot = SearchLoopSnapshot::from_authoritative_state(committed.wire())
        .expect("search loop snapshot must derive from authoritative state");
    let ActiveProgram::Admitted { program, .. } = committed.active_program() else {
        panic!("test state must retain an admitted program");
    };
    assert_eq!(
        SearchLoopReducer::advance(program, &snapshot).expect("joined program must advance"),
        SearchLoopDirective::EvaluateClosure
    );
    let commits = commits.lock().expect("commit log");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].events().len(), 1);
}

#[test]
fn parallel_group_lifecycle_fans_out_and_fans_in_without_partial_mvcc_state() {
    let record = authoritative_record_for_program(Vec::new(), "parallel", 2);
    let (router, commits) = router_with_commits(record.clone());
    let driver = StaticExecutionDriver::default();
    let joined = block_on(router.advance_search_loop(advance_request(&record), &driver))
        .expect("one search-loop advance command must complete the admitted parallel group");

    assert_eq!(joined.revision(), record.state.revision + 5);
    assert!(
        joined
            .executions()
            .iter()
            .all(|execution| matches!(execution, ExecutionAuthority::Consumed(_)))
    );
    assert_eq!(joined.wire().joined_execution_groups.len(), 1);
    let calls = driver.calls.lock().expect("driver calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].len(), 2);
    let commits = commits.lock().expect("commit log");
    assert_eq!(commits.len(), 5);
    assert_eq!(
        commits
            .iter()
            .map(|commit| commit.events().len())
            .collect::<Vec<_>>(),
        [2, 2, 2, 2, 1]
    );
}

#[test]
fn poll_resumes_in_flight_group_without_replaying_prior_phases() {
    let record = authoritative_record_for_program(Vec::new(), "parallel", 2);
    let (router, commits) = router_with_commits(record.clone());
    let error =
        block_on(router.advance_search_loop(advance_request(&record), &FailingExecutionDriver))
            .expect_err("provider failure must leave the group at the in-flight recovery point");
    assert!(matches!(error, GraphRouterError::Provider(_)));
    let in_flight = {
        let commits = commits.lock().expect("commit log");
        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.events().len())
                .collect::<Vec<_>>(),
            [2, 2, 2]
        );
        commits[2].next_state().clone()
    };
    let joined = block_on(router.poll_search_loop(
        SearchLoopPollRequest {
            run_id: in_flight.run_id.clone(),
            expected_revision: in_flight.revision,
            expected_state_digest: in_flight.state_digest.clone(),
            expected_context_binding_digest: in_flight.context.binding_digest.clone(),
            execution_group_id: id("group-1"),
            dispatches: vec![
                SearchLoopPollDispatch {
                    stage_ids: vec![id("stage-1")],
                    consume_event_id: id("event-poll-consume-1"),
                },
                SearchLoopPollDispatch {
                    stage_ids: vec![id("stage-2")],
                    consume_event_id: id("event-poll-consume-2"),
                },
            ],
            join_event_id: id("event-poll-join"),
        },
        &StaticExecutionDriver::default(),
    ))
    .expect("poll must resume the existing attempts through consume and join");
    assert_eq!(joined.revision(), record.state.revision + 5);
    let commits = commits.lock().expect("commit log");
    assert_eq!(
        commits
            .iter()
            .map(|commit| commit.events().len())
            .collect::<Vec<_>>(),
        [2, 2, 2, 2, 1]
    );
}

#[test]
fn parallel_advance_fills_available_slot_without_consuming_existing_attempt() {
    let record = authoritative_record_for_program(vec![in_flight_execution()], "parallel", 3);
    let (router, commits) = router_with_commits(record.clone());
    let advanced = block_on(router.advance_search_loop(
        SearchLoopAdvanceRequest {
            run_id: record.state.run_id.clone(),
            expected_revision: record.state.revision,
            expected_state_digest: record.state.state_digest.clone(),
            expected_context_binding_digest: record.state.context.binding_digest.clone(),
            directive: SearchLoopDirective::AdmitParallel {
                group_id: id("group-1"),
                stage_ids: vec![id("stage-2")],
                max_parallel: 3,
                independence_proof_ref: id("independence-proof-1"),
            },
            dispatches: vec![advance_dispatch(2)],
            join_event_id: id("event-partial-wave-join"),
        },
        &StaticExecutionDriver::default(),
    ))
    .expect("available parallel slot must admit and consume only its new attempt");

    assert_eq!(advanced.revision(), record.state.revision + 4);
    assert!(advanced.wire().joined_execution_groups.is_empty());
    assert!(advanced.executions().iter().any(|execution| {
        execution.stage_ids() == [id("stage-1")]
            && matches!(execution, ExecutionAuthority::InFlight(_))
    }));
    assert!(advanced.executions().iter().any(|execution| {
        execution.stage_ids() == [id("stage-2")]
            && matches!(execution, ExecutionAuthority::Consumed(_))
    }));
    let commits = commits.lock().expect("commit log");
    assert_eq!(
        commits
            .iter()
            .map(|commit| commit.events().len())
            .collect::<Vec<_>>(),
        [1, 1, 1, 1]
    );
}

fn advance_request(record: &AuthoritativeStateRecord) -> SearchLoopAdvanceRequest {
    SearchLoopAdvanceRequest {
        run_id: record.state.run_id.clone(),
        expected_revision: record.state.revision,
        expected_state_digest: record.state.state_digest.clone(),
        expected_context_binding_digest: record.state.context.binding_digest.clone(),
        directive: SearchLoopDirective::AdmitParallel {
            group_id: id("group-1"),
            stage_ids: vec![id("stage-1"), id("stage-2")],
            max_parallel: 2,
            independence_proof_ref: id("independence-proof-1"),
        },
        dispatches: vec![advance_dispatch(1), advance_dispatch(2)],
        join_event_id: id("event-lifecycle-join"),
    }
}

fn advance_dispatch(index: usize) -> SearchLoopAdvanceDispatch {
    SearchLoopAdvanceDispatch {
        stage_ids: vec![id(&format!("stage-{index}"))],
        input_facts_digest: Digest::from_bytes(format!("lifecycle-input-{index}").as_bytes()),
        effect_class: EffectClass::ReadOnly,
        lease_duration_ms: 1_000,
        admission_event_id: id(&format!("event-lifecycle-admit-{index}")),
        grant_event_id: id(&format!("event-lifecycle-grant-{index}")),
        start_event_id: id(&format!("event-lifecycle-start-{index}")),
        consume_event_id: id(&format!("event-lifecycle-consume-{index}")),
    }
}

#[derive(Default)]
struct StaticExecutionDriver {
    calls: Mutex<Vec<Vec<ProviderExecutionDispatch>>>,
}

impl SearchExecutionDriver for StaticExecutionDriver {
    type Error = String;

    fn execute_group<'a>(
        &'a self,
        _execution_group_id: &'a ProtocolId,
        dispatches: Vec<ProviderExecutionDispatch>,
    ) -> PortFuture<'a, Vec<ProviderExecutionResult>, Self::Error> {
        Box::pin(async move {
            self.calls
                .lock()
                .map_err(|error| error.to_string())?
                .push(dispatches.clone());
            Ok(dispatches
                .into_iter()
                .enumerate()
                .map(|(index, dispatch)| ProviderExecutionResult {
                    attempt_id: dispatch.attempt_id,
                    result_receipt_ref: id(&format!("result-lifecycle-{}", index + 1)),
                    result_digest: Digest::from_bytes(
                        format!("result-digest-{}", index + 1).as_bytes(),
                    ),
                })
                .collect())
        })
    }
}

struct FailingExecutionDriver;

impl SearchExecutionDriver for FailingExecutionDriver {
    type Error = String;

    fn execute_group<'a>(
        &'a self,
        _execution_group_id: &'a ProtocolId,
        _dispatches: Vec<ProviderExecutionDispatch>,
    ) -> PortFuture<'a, Vec<ProviderExecutionResult>, Self::Error> {
        Box::pin(async { Err("provider unavailable".to_owned()) })
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
    let record = authoritative_record(vec![granted_execution()]);
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
    let record = authoritative_record(vec![consumed_execution()]);
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
    let record = authoritative_record(vec![consumed_execution()]);
    let request = closure_request(&record);
    let router = router(record);

    let committed = block_on(router.finalize_closure(request))
        .expect("terminal execution and matching proof must close");
    assert!(matches!(
        committed.wire().closure,
        ClosureDisposition::Finalized { .. }
    ));
    assert!(matches!(
        committed.wire().executions.as_slice(),
        [ExecutionAuthority::Consumed(_)]
    ));
}

#[test]
fn closure_rejects_consumed_group_without_persisted_join() {
    let record = authoritative_record_for_program(vec![consumed_execution()], "serial", 1);
    let request = closure_request(&record);
    let router = router(record);

    let error = block_on(router.finalize_closure(request))
        .expect_err("terminal provider execution alone must not authorize closure");
    assert!(matches!(
        error,
        GraphRouterError::InvalidTransition("search closure requires every execution group joined")
    ));
}

fn router(
    record: AuthoritativeStateRecord,
) -> GraphRouter<MemoryStore, StaticProofResolver, FixedClock> {
    router_with_commits(record).0
}

fn router_with_commits(record: AuthoritativeStateRecord) -> (TestRouter, CommitLog) {
    let receipt = evidence_receipt(&record.state);
    let commits = Arc::new(Mutex::new(Vec::new()));
    let router = GraphRouter::new(
        MemoryStore {
            record: Mutex::new(record),
            commits: Arc::clone(&commits),
        },
        StaticProofResolver { receipt },
        FixedClock(100),
    );
    (router, commits)
}

fn admit_directive_request(
    record: &AuthoritativeStateRecord,
    directive: SearchLoopDirective,
    dispatches: Vec<ExecutionDispatchAdmission>,
) -> AdmitSearchLoopDirectiveRequest {
    AdmitSearchLoopDirectiveRequest {
        run_id: record.state.run_id.clone(),
        expected_revision: record.state.revision,
        expected_state_digest: record.state.state_digest.clone(),
        expected_context_binding_digest: record.state.context.binding_digest.clone(),
        directive,
        dispatches,
    }
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

fn authoritative_record(executions: Vec<ExecutionAuthority>) -> AuthoritativeStateRecord {
    let mut record = authoritative_record_for_program(executions, "serial", 1);
    if !record.state.executions.is_empty()
        && record
            .state
            .executions
            .iter()
            .all(|execution| matches!(execution, ExecutionAuthority::Consumed { .. }))
    {
        add_joined_group(&mut record);
    }
    record
}

fn authoritative_record_for_program(
    mut executions: Vec<ExecutionAuthority>,
    mode: &str,
    stage_count: usize,
) -> AuthoritativeStateRecord {
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
    let stages = (1..=stage_count)
        .map(|index| {
            serde_json::json!({
                "stageId": format!("stage-{index}"),
                "proposalNodeId": format!("node-{index}"),
                "providerId": "provider-1",
                "catalogId": "search",
                "inputTemplateDigest": digest(&format!("input-template-{index}")),
                "coversObligationIds": ["obligation-1"],
                "requiredClosure": "discovery",
                "evidencePredicate": {
                    "predicateId": format!("predicate-{index}"),
                    "claimClass": "identity",
                    "scopeDigest": digest("scope"),
                    "acceptedSchemaIds": ["semantic-search-packet.v1"],
                    "requiredFields": ["symbol"],
                    "completeness": "complete-scope",
                    "requiresFreshBinding": true,
                    "requiresUntruncated": true,
                    "maxResults": 8
                }
            })
        })
        .collect::<Vec<_>>();
    let stage_ids = (1..=stage_count)
        .map(|index| format!("stage-{index}"))
        .collect::<Vec<_>>();
    let mut execution_group = serde_json::json!({
        "groupId": "group-1",
        "mode": mode,
        "stageIds": stage_ids,
        "joinPolicy": "all-required",
        "maxParallel": if mode == "parallel" { stage_count } else { 1 },
        "derivationReceiptRef": "derivation-1"
    });
    if mode == "batch" {
        execution_group["batchCapabilityRef"] = serde_json::json!("batch-capability-1");
    } else if mode == "parallel" {
        execution_group["independenceProofRef"] = serde_json::json!("independence-proof-1");
    }
    let mut program: RouteProgram = serde_json::from_value(serde_json::json!({
        "programId": "program-1",
        "proposalId": "proposal-1",
        "runId": state.run_id.as_str(),
        "admittedAtRevision": 1,
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "graphDigest": digest("pending-graph"),
        "catalogDigest": digest("catalog"),
        "stages": stages,
        "executionGroups": [execution_group],
        "edges": [],
        "joins": [],
        "budgetLimit": {
            "maxCommands": 4,
            "maxElapsedMs": 10000,
            "maxPacketBytes": 65536,
            "maxChoiceDepth": 4,
            "maxParallel": 4,
            "maxAggregateProviderLatencyMs": 20000,
            "maxParentVisibleBytes": 65536
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
    for execution in &mut executions {
        match execution {
            ExecutionAuthority::Admitted(_)
            | ExecutionAuthority::Granted(_)
            | ExecutionAuthority::InFlight(_)
            | ExecutionAuthority::Consumed(_)
            | ExecutionAuthority::Revoked(_) => {
                let program_digest = execution.program_digest_mut();
                *program_digest = program.program_digest.clone();
            }
        }
    }
    state.executions = executions;
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
        search_loop_capabilities: Vec::new(),
        search_loop_runtime: None,
    }
}

fn add_joined_group(record: &mut AuthoritativeStateRecord) {
    let mut result_receipt_refs = record
        .state
        .executions
        .iter()
        .filter_map(|execution| match execution {
            ExecutionAuthority::Consumed(_) => Some(
                execution
                    .result_receipt_ref()
                    .expect("consumed authority has result receipt")
                    .clone(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    result_receipt_refs.sort();
    result_receipt_refs.dedup();
    let mut joined = JoinedExecutionGroup {
        execution_group_id: id("group-1"),
        policy: JoinPolicy::AllRequired,
        result_receipt_refs,
        join_digest: Digest::from_bytes(b"pending-join"),
    };
    joined.join_digest = joined.recompute_join_digest();
    record.state.joined_execution_groups = vec![joined];
    record.state.state_digest = record.state.recompute_state_digest();
    record.authority_receipt.state_digest = record.state.state_digest.clone();
    record.authority_receipt.receipt_digest = record.authority_receipt.recompute_receipt_digest();
    record.state.validate().expect("joined state must validate");
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
        "executionGroupId": "group-1",
        "stageIds": ["stage-1"],
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
        "programDigest": digest("program"),
        "executionGroupId": "group-1",
        "stageIds": ["stage-1"],
        "providerId": "provider-1",
        "operation": "search",
        "resultReceiptRef": "result-1",
        "resultDigest": digest("result"),
        "leaseFence": 1
    }))
    .expect("consumed execution must deserialize")
}

fn in_flight_execution() -> ExecutionAuthority {
    serde_json::from_value(serde_json::json!({
        "kind": "in-flight",
        "admissionId": "admission-existing",
        "admissionDigest": digest("admission-existing"),
        "grantId": "grant-existing",
        "grantDigest": digest("grant-existing"),
        "actionKey": digest("action-existing"),
        "attemptId": "attempt-existing",
        "attemptDigest": digest("attempt-existing"),
        "programDigest": digest("program"),
        "executionGroupId": "group-1",
        "stageIds": ["stage-1"],
        "providerId": "provider-1",
        "operation": "search",
        "resolvedInputDigest": digest("input-existing"),
        "policyDigest": digest("policy-existing"),
        "dependencyDigest": digest("dependency-existing"),
        "budgetReservationId": "budget-existing",
        "budgetChargeKey": digest("charge-existing"),
        "effectClass": effect_class(),
        "leaseFence": 1,
        "expiresAtMs": 1000,
        "providerIdempotencyKey": "idempotency-existing"
    }))
    .expect("in-flight execution must deserialize")
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
