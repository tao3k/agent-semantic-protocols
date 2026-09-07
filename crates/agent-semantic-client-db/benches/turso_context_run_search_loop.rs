// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_client_db::context_run_mvcc::TursoMvccContextRunStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStore;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig;
use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::EffectClass;
use agent_semantic_context_product::EvidenceReceipt;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_loop::GraphRouter;
use agent_semantic_loop::PortFuture;
use agent_semantic_loop::ProofResolver;
use agent_semantic_loop::ProviderExecutionDispatch;
use agent_semantic_loop::ProviderExecutionResult;
use agent_semantic_loop::SearchExecutionDriver;
use agent_semantic_loop::SearchLoopAdvanceDispatch;
use agent_semantic_loop::SearchLoopAdvanceRequest;
use agent_semantic_loop::TrustedClock;
use agent_semantic_loop::search_loop::SearchLoopDirective;
use criterion::BatchSize;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;

const PARALLEL_DISPATCHES: usize = 2;

struct NoProofResolver;

impl ProofResolver for NoProofResolver {
    type Error = String;

    fn resolve<'a>(
        &'a self,
        _proof_ref: &'a ProtocolId,
    ) -> PortFuture<'a, EvidenceReceipt, Self::Error> {
        Box::pin(async { Err("benchmark does not resolve closure proofs".to_owned()) })
    }
}

#[derive(Clone, Copy)]
struct FixedClock(u64);

impl TrustedClock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

struct ImmediateDriver;

impl SearchExecutionDriver for ImmediateDriver {
    type Error = String;

    fn execute_group<'a>(
        &'a self,
        _execution_group_id: &'a ProtocolId,
        dispatches: Vec<ProviderExecutionDispatch>,
    ) -> PortFuture<'a, Vec<ProviderExecutionResult>, Self::Error> {
        Box::pin(async move {
            Ok(dispatches
                .into_iter()
                .map(|dispatch| ProviderExecutionResult {
                    result_receipt_ref: id(&format!(
                        "result-{}",
                        dispatch
                            .attempt_id
                            .as_str()
                            .trim_start_matches("execution-attempt:")
                    )),
                    result_digest: Digest::from_bytes(dispatch.attempt_id.as_str().as_bytes()),
                    attempt_id: dispatch.attempt_id,
                })
                .collect())
        })
    }
}

fn benchmark(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build context-run benchmark runtime");
    let temp =
        std::env::temp_dir().join(format!("asp-context-run-loop-bench-{}", std::process::id()));
    std::fs::create_dir_all(&temp).expect("create context-run benchmark directory");
    let mvcc = runtime
        .block_on(TursoMvccStore::open(TursoMvccStoreConfig::new(
            temp.join("context-run.turso"),
        )))
        .expect("open Turso MVCC store");
    let run_store = TursoMvccContextRunStore::new(mvcc, id("benchmark-authority"));
    let sequence = AtomicU64::new(0);

    let mut group = criterion.benchmark_group("turso_context_run_search_loop");
    group.throughput(criterion::Throughput::Elements(PARALLEL_DISPATCHES as u64));
    group.bench_function("parallel_2_dispatch_full_lifecycle", |bencher| {
        bencher.iter_batched(
            || {
                let run = sequence.fetch_add(1, Ordering::Relaxed);
                let state = initial_state(run);
                let search_loop_runtime = initial_search_loop_runtime(&state);
                let loop_id = search_loop_runtime.loop_id().clone();
                runtime
                    .block_on(run_store.initialize(
                        state.clone(),
                        Vec::new(),
                        Some(search_loop_runtime),
                        1_000,
                    ))
                    .expect("initialize benchmark context run");
                (state, loop_id)
            },
            |(state, loop_id)| {
                let resolved = runtime
                    .block_on(run_store.load_search_loop_runtime(&loop_id))
                    .expect("resolve benchmark loop alias")
                    .expect("benchmark loop runtime");
                assert_eq!(resolved.state.run_id, state.run_id);
                let router =
                    GraphRouter::new(run_store.clone(), NoProofResolver, FixedClock(2_000));
                runtime
                    .block_on(
                        router.advance_search_loop(
                            advance_request(&resolved.state),
                            &ImmediateDriver,
                        ),
                    )
                    .expect("advance benchmark search loop");
            },
            BatchSize::SmallInput,
        );
    });
    group.throughput(criterion::Throughput::Elements(
        (4 * PARALLEL_DISPATCHES) as u64,
    ));
    group.bench_function("concurrent_4_runs_x2_dispatches", |bencher| {
        bencher.iter_batched(
            || {
                std::array::from_fn(|_| {
                    let state = initial_state(sequence.fetch_add(1, Ordering::Relaxed));
                    let search_loop_runtime = initial_search_loop_runtime(&state);
                    let loop_id = search_loop_runtime.loop_id().clone();
                    runtime
                        .block_on(run_store.initialize(
                            state.clone(),
                            Vec::new(),
                            Some(search_loop_runtime),
                            1_000,
                        ))
                        .expect("initialize concurrent benchmark context run");
                    (state, loop_id)
                })
            },
            |runs: [(UncheckedContextProductStateV1, ProtocolId); 4]| {
                let resolved = runtime.block_on(async {
                    tokio::join!(
                        run_store.load_search_loop_runtime(&runs[0].1),
                        run_store.load_search_loop_runtime(&runs[1].1),
                        run_store.load_search_loop_runtime(&runs[2].1),
                        run_store.load_search_loop_runtime(&runs[3].1),
                    )
                });
                let states = [
                    resolved.0.expect("resolve loop 1").expect("loop 1").state,
                    resolved.1.expect("resolve loop 2").expect("loop 2").state,
                    resolved.2.expect("resolve loop 3").expect("loop 3").state,
                    resolved.3.expect("resolve loop 4").expect("loop 4").state,
                ];
                let routers: [_; 4] = std::array::from_fn(|_| {
                    GraphRouter::new(run_store.clone(), NoProofResolver, FixedClock(2_000))
                });
                let driver = ImmediateDriver;
                let results = runtime.block_on(async {
                    tokio::join!(
                        routers[0].advance_search_loop(advance_request(&states[0]), &driver),
                        routers[1].advance_search_loop(advance_request(&states[1]), &driver),
                        routers[2].advance_search_loop(advance_request(&states[2]), &driver),
                        routers[3].advance_search_loop(advance_request(&states[3]), &driver),
                    )
                });
                results
                    .0
                    .expect("advance concurrent benchmark search loop 1");
                results
                    .1
                    .expect("advance concurrent benchmark search loop 2");
                results
                    .2
                    .expect("advance concurrent benchmark search loop 3");
                results
                    .3
                    .expect("advance concurrent benchmark search loop 4");
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();

    drop(run_store);
    let _ = std::fs::remove_dir_all(temp);
}

fn initial_state(run: u64) -> UncheckedContextProductStateV1 {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/context-product-state.v1.fixtures.json"
    ))
    .expect("parse context-product fixtures");
    let mut state: UncheckedContextProductStateV1 = serde_json::from_value(
        fixtures["fixtures"]
            .as_array()
            .expect("fixtures array")
            .iter()
            .find(|fixture| fixture["name"] == "valid-open-search-state")
            .expect("open state fixture")["value"]
            .clone(),
    )
    .expect("deserialize open state");
    state.run_id = id(&format!("benchmark-run-{run}"));
    state.authority_receipt_ref = id(&format!("benchmark-authority-receipt-{run}"));
    state.context.binding_digest = state.context.recompute_binding_digest();
    let mut program: RouteProgram = serde_json::from_value(serde_json::json!({
        "programId": format!("benchmark-program-{run}"),
        "proposalId": format!("benchmark-proposal-{run}"),
        "runId": state.run_id.as_str(),
        "admittedAtRevision": 0,
        "contextBindingDigest": state.context.binding_digest.as_str(),
        "graphDigest": digest("pending-graph"),
        "catalogDigest": digest("catalog"),
        "stages": [route_stage(1), route_stage(2)],
        "executionGroups": [{
            "groupId": "group-1",
            "mode": "parallel",
            "stageIds": ["stage-1", "stage-2"],
            "joinPolicy": "all-required",
            "maxParallel": 2,
            "derivationReceiptRef": "derivation-1",
            "independenceProofRef": "independence-proof-1"
        }],
        "edges": [],
        "joins": [],
        "budgetLimit": {
            "maxCommands": 8,
            "maxElapsedMs": 30000,
            "maxPacketBytes": 131072,
            "maxChoiceDepth": 4,
            "maxParallel": 2,
            "maxAggregateProviderLatencyMs": 30000,
            "maxParentVisibleBytes": 131072
        },
        "programDigest": digest("pending-program")
    }))
    .expect("deserialize benchmark route program");
    program.graph_digest = program.recompute_graph_digest();
    program.program_digest = program.recompute_program_digest();
    state.active_program = ActiveProgram::Admitted {
        proposal_id: program.proposal_id.clone(),
        proposal_digest: Digest::from_bytes(b"benchmark-proposal"),
        intent_digest: Digest::from_bytes(b"benchmark-intent"),
        program_id: program.program_id.clone(),
        program_digest: program.program_digest.clone(),
        graph_digest: program.graph_digest.clone(),
        admitted_at_revision: 0,
        context_binding_digest: state.context.binding_digest.clone(),
        program: Box::new(program),
    };
    state.frontier.graph_digest = match &state.active_program {
        ActiveProgram::Admitted { graph_digest, .. } => graph_digest.clone(),
        ActiveProgram::None => unreachable!(),
    };
    state.frontier.context_binding_digest = state.context.binding_digest.clone();
    state.frontier.frontier_digest = state.frontier.recompute_frontier_digest();
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();
    state.validate().expect("benchmark state must validate");
    state
}

fn initial_search_loop_runtime(
    state: &UncheckedContextProductStateV1,
) -> agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1 {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/search-interactive-loop/runtime-active.v1.json"
    ))
    .expect("runtime fixture");
    value["loopId"] = serde_json::json!(format!("loop:{}", state.run_id.as_str()));
    value["runId"] = serde_json::json!(state.run_id.as_str());
    value["contextBindingDigest"] = serde_json::json!(state.context.binding_digest.as_str());
    value["openedAtRevision"] = serde_json::json!(0);
    value["activePanel"]["basedOnRevision"] = serde_json::json!(0);
    value["batches"] = serde_json::json!([]);
    let unchecked = serde_json::from_value(value).expect("unchecked runtime fixture");
    agent_semantic_loop::search_runtime::SearchLoopRuntimeBindingV1::validate(unchecked)
        .expect("valid benchmark runtime")
}

fn route_stage(index: usize) -> serde_json::Value {
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
}

fn advance_request(state: &UncheckedContextProductStateV1) -> SearchLoopAdvanceRequest {
    SearchLoopAdvanceRequest {
        run_id: state.run_id.clone(),
        expected_revision: state.revision,
        expected_state_digest: state.state_digest.clone(),
        expected_context_binding_digest: state.context.binding_digest.clone(),
        directive: SearchLoopDirective::AdmitParallel {
            group_id: id("group-1"),
            stage_ids: vec![id("stage-1"), id("stage-2")],
            max_parallel: 2,
            independence_proof_ref: id("independence-proof-1"),
        },
        dispatches: (1..=PARALLEL_DISPATCHES)
            .map(|index| SearchLoopAdvanceDispatch {
                stage_ids: vec![id(&format!("stage-{index}"))],
                input_facts_digest: Digest::from_bytes(format!("input-facts-{index}").as_bytes()),
                effect_class: EffectClass::ReadOnly,
                lease_duration_ms: 10_000,
                admission_event_id: id(&format!("admit-event-{index}")),
                grant_event_id: id(&format!("grant-event-{index}")),
                start_event_id: id(&format!("start-event-{index}")),
                consume_event_id: id(&format!("consume-event-{index}")),
            })
            .collect(),
        join_event_id: id("join-event-1"),
    }
}

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("valid benchmark protocol id")
}

fn digest(value: &str) -> String {
    Digest::from_bytes(value.as_bytes()).as_str().to_owned()
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
