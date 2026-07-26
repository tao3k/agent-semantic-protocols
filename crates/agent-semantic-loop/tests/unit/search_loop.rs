use agent_semantic_context_product::{
    ClaimClass, EvidenceCompleteness, EvidencePredicate, RequiredClosure, RouteEdge,
    RouteExecutionGroup, RouteStage, SearchBudget,
};

use super::{
    BTreeSet, Digest, JoinPolicy, ProtocolId, RouteExecutionMode, RouteProgram,
    SearchLoopDirective, SearchLoopReducer, SearchLoopSnapshot, StageExecutionStatus,
};

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("valid protocol id")
}

fn stage(value: &str) -> RouteStage {
    RouteStage {
        stage_id: id(value),
        proposal_node_id: id(&format!("proposal-{value}")),
        provider_id: id("rust-provider"),
        catalog_id: id("owner-items.v1"),
        input_template_digest: Digest::from_bytes(value.as_bytes()),
        covers_obligation_ids: vec![id(&format!("obligation-{value}"))],
        required_closure: RequiredClosure::Existential,
        evidence_predicate: EvidencePredicate {
            predicate_id: id(&format!("predicate-{value}")),
            claim_class: ClaimClass::Existential,
            scope_digest: Digest::from_bytes(b"scope"),
            accepted_schema_ids: vec![id("semantic-search-packet.v1")],
            required_fields: vec![id("canonicalItemSelector")],
            completeness: EvidenceCompleteness::IdentityOnly,
            requires_fresh_binding: true,
            requires_untruncated: false,
            max_results: 10,
        },
    }
}

fn program(
    mode: RouteExecutionMode,
    stage_names: &[&str],
    edges: &[(&str, &str)],
    max_parallel: u64,
) -> RouteProgram {
    let stages: Vec<_> = stage_names.iter().map(|name| stage(name)).collect();
    let group = RouteExecutionGroup {
        group_id: id("G1"),
        mode,
        stage_ids: stages.iter().map(|stage| stage.stage_id.clone()).collect(),
        join_policy: JoinPolicy::AllRequired,
        max_parallel,
        derivation_receipt_ref: id("derivation.group.G1"),
        batch_capability_ref: (mode == RouteExecutionMode::Batch)
            .then(|| id("rust.owner-items.batch.v1")),
        independence_proof_ref: (mode == RouteExecutionMode::Parallel)
            .then(|| id("proof.independent.G1")),
    };
    RouteProgram {
        program_id: id("PROGRAM"),
        proposal_id: id("PROPOSAL"),
        run_id: id("RUN"),
        admitted_at_revision: 1,
        context_binding_digest: Digest::from_bytes(b"context"),
        graph_digest: Digest::from_bytes(b"graph"),
        catalog_digest: Digest::from_bytes(b"catalog"),
        stages,
        execution_groups: vec![group],
        edges: edges
            .iter()
            .map(|(from, to)| RouteEdge {
                from: id(from),
                to: id(to),
            })
            .collect(),
        joins: Vec::new(),
        budget_limit: SearchBudget {
            max_commands: 10.into(),
            max_elapsed_ms: 1_000.into(),
            max_packet_bytes: 10_000.into(),
            max_choice_depth: 4.into(),
            max_parallel: 10.into(),
            max_aggregate_provider_latency_ms: 3_000.into(),
            max_parent_visible_bytes: 2_000.into(),
        },
        program_digest: Digest::from_bytes(b"program"),
    }
}

fn snapshot(program: &RouteProgram) -> SearchLoopSnapshot {
    SearchLoopSnapshot {
        program_id: program.program_id.clone(),
        program_digest: program.program_digest.clone(),
        stages: program
            .stages
            .iter()
            .map(|stage| (stage.stage_id.clone(), StageExecutionStatus::Pending))
            .collect(),
        joined_group_ids: BTreeSet::new(),
    }
}

#[test]
fn parallel_choice_becomes_one_multi_point_directive() {
    let program = program(
        RouteExecutionMode::Parallel,
        &["N-IMPL", "N-TEST", "N-POLICY"],
        &[],
        3,
    );
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot(&program)),
        Ok(SearchLoopDirective::AdmitParallel {
            group_id: id("G1"),
            stage_ids: vec![id("N-IMPL"), id("N-TEST"), id("N-POLICY")],
            max_parallel: 3,
            independence_proof_ref: id("proof.independent.G1"),
        })
    );
}

#[test]
fn batch_choice_becomes_one_provider_dispatch() {
    let program = program(RouteExecutionMode::Batch, &["N1", "N2"], &[], 1);
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot(&program)),
        Ok(SearchLoopDirective::AdmitBatch {
            group_id: id("G1"),
            stage_ids: vec![id("N1"), id("N2")],
            batch_capability_ref: id("rust.owner-items.batch.v1"),
        })
    );
}

#[test]
fn serial_group_advances_from_graph_dependencies() {
    let program = program(
        RouteExecutionMode::Serial,
        &["N1", "N2"],
        &[("N1", "N2")],
        1,
    );
    let mut snapshot = snapshot(&program);
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot),
        Ok(SearchLoopDirective::AdmitSerial {
            group_id: id("G1"),
            stage_id: id("N1"),
        })
    );
    snapshot.stages.insert(
        id("N1"),
        StageExecutionStatus::Succeeded {
            result_receipt_ref: id("receipt.N1"),
        },
    );
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot),
        Ok(SearchLoopDirective::AdmitSerial {
            group_id: id("G1"),
            stage_id: id("N2"),
        })
    );
}

#[test]
fn running_parallel_group_fills_only_available_slots() {
    let program = program(RouteExecutionMode::Parallel, &["N1", "N2", "N3"], &[], 2);
    let mut snapshot = snapshot(&program);
    snapshot.stages.insert(
        id("N1"),
        StageExecutionStatus::Running {
            attempt_ref: id("attempt.N1"),
        },
    );
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot),
        Ok(SearchLoopDirective::AdmitParallel {
            group_id: id("G1"),
            stage_ids: vec![id("N2")],
            max_parallel: 2,
            independence_proof_ref: id("proof.independent.G1"),
        })
    );
}

#[test]
fn joined_group_advances_to_closure_evaluation() {
    let program = program(RouteExecutionMode::Serial, &["N1"], &[], 1);
    let mut snapshot = snapshot(&program);
    snapshot.stages.insert(
        id("N1"),
        StageExecutionStatus::Succeeded {
            result_receipt_ref: id("receipt.N1"),
        },
    );
    assert!(matches!(
        SearchLoopReducer::advance(&program, &snapshot),
        Ok(SearchLoopDirective::EvaluateJoin { .. })
    ));
    snapshot.joined_group_ids.insert(id("G1"));
    assert_eq!(
        SearchLoopReducer::advance(&program, &snapshot),
        Ok(SearchLoopDirective::EvaluateClosure)
    );
}

#[test]
fn stage_statuses_cover_the_authoritative_execution_lifecycle() {
    let admitted = StageExecutionStatus::Admitted {
        admission_ref: id("admission.N1"),
    };
    let granted = StageExecutionStatus::Granted {
        grant_ref: id("grant.N1"),
    };
    let failed = StageExecutionStatus::Failed {
        failure_receipt_ref: id("failure.N1"),
    };
    let cancelled = StageExecutionStatus::Cancelled {
        cancellation_receipt_ref: id("cancellation.N1"),
    };
    assert!(admitted.is_active());
    assert!(granted.is_active());
    assert!(!failed.is_active());
    assert!(!cancelled.is_active());
}
