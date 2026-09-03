use agent_semantic_search::{
    NativeSyntaxProjection, NativeSyntaxRelation, NativeSyntaxSelector, ResidentGraphSearchStage,
    ResidentGraphSearchWork, SearchPlaybookColdRgExecution, SearchPlaybookPythonGraphExecution,
    SearchPlaybookReceiptInput, build_search_playbook_receipt,
};
use agent_semantic_search_projection::{ResidentSearchWorkCounters, RuntimeProviderSearchReceipt};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn runtime_receipt() -> RuntimeProviderSearchReceipt {
    RuntimeProviderSearchReceipt {
        schema_id: "agent.semantic-protocols.runtime-provider-search-receipt".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "search-1".to_owned(),
        status: "matches".to_owned(),
        language_id: "rust".to_owned(),
        generation_digest: digest('1'),
        root_digest: digest('2'),
        provider_digest: digest('3'),
        index_artifact_digest: digest('4'),
        candidate_count: 1,
        selector_projection_budget: 16,
        projected_owner_count: 1,
        selectors: vec!["rust://src/lib.rs#item/function/run".to_owned()],
        owner_paths: vec!["src/lib.rs".to_owned()],
        resident_read_elapsed_micros: 7,
        service_elapsed_micros: 11,
        elapsed_micros: 18,
        work_counters: ResidentSearchWorkCounters::default(),
    }
}

fn input() -> SearchPlaybookReceiptInput {
    SearchPlaybookReceiptInput {
        workspace_identity: "workspace-test".to_owned(),
        query: "run".to_owned(),
        intent: "conceptual".to_owned(),
        coverage: "candidates".to_owned(),
        max_owners: 16,
        deadline_ms: 500,
        indexed_owner_count: 12,
        indexed_lexical_executed: true,
        resident_graph_executed: true,
        byte_evidence_executed: false,
        source_acquisition_stage_artifact_digest: digest('a'),
        native_syntax_state: "ready".to_owned(),
        native_syntax_stage_artifact_digest: digest('5'),
        native_syntax_projections: vec![NativeSyntaxProjection {
            owner_path: "src/lib.rs".to_owned(),
            content_digest: digest('6'),
            selectors: vec![NativeSyntaxSelector {
                selector: "rust://src/lib.rs#item/function/run".to_owned(),
                byte_start: 3,
                byte_end: 19,
                query_keys: vec!["run".to_owned()],
                derived_projection_digest: digest('7'),
            }],
        }],
        native_syntax_relations: vec![NativeSyntaxRelation {
            owner_path: "src/lib.rs".to_owned(),
            relation_digest: digest('8'),
        }],
        native_syntax_elapsed_micros: 2,
        runtime: runtime_receipt(),
        graph: ResidentGraphSearchStage {
            generation_digest: digest('1'),
            result_digest: digest('9'),
            ranked_owner_paths: vec!["src/lib.rs".to_owned()],
            elapsed_micros: 11,
            work: ResidentGraphSearchWork::default(),
        },
        cold_rg: None,
        python_graph: None,
    }
}

#[test]
fn production_playbook_receipt_projects_native_syntax_not_owner_only_evidence() {
    let receipt = build_search_playbook_receipt(input()).expect("complete playbook receipt");
    assert_eq!(receipt.schema_id, "asp.search.playbook-receipt");
    assert_eq!(receipt.plan.stages.len(), 6);
    assert_eq!(receipt.evidence.native_syntax.projections.len(), 1);
    assert_eq!(
        receipt.evidence.native_syntax.projections[0].selectors[0].query_keys,
        ["run"]
    );
    assert_eq!(receipt.decision.selectors.len(), 1);
    assert_eq!(receipt.evidence.indexed_lexical.state, "executed");
    assert_eq!(receipt.evidence.byte_evidence.state, "skipped");
    assert_eq!(receipt.evidence.ripgrep.state, "skipped");
    assert!(!receipt.evidence.ripgrep.complete);
    assert!(receipt.evidence.ripgrep.coverage_input_digest.is_none());
    assert_eq!(
        receipt.evidence.resident_graph.backend,
        "rust-resident-graph"
    );
    assert_eq!(receipt.evidence.python_graph.state, "skipped");
}

#[test]
fn cold_rg_receipt_records_one_exact_generation_process_without_tantivy() {
    let mut input = input();
    input.indexed_lexical_executed = false;
    input.resident_graph_executed = false;
    input.cold_rg = Some(SearchPlaybookColdRgExecution {
        generation_digest: digest('1'),
        coverage_input_digest: digest('a'),
        candidate_owner_ids: vec!["src/lib.rs".to_owned()],
        elapsed_micros: 41,
        process_count: 1,
    });

    let receipt = build_search_playbook_receipt(input).expect("cold rg playbook receipt");
    assert_eq!(receipt.evidence.indexed_lexical.state, "skipped");
    assert_eq!(receipt.evidence.ripgrep.state, "executed");
    assert_eq!(receipt.evidence.ripgrep.process_count, 1);
    assert_eq!(receipt.evidence.ripgrep.candidate_owner_ids, ["src/lib.rs"]);
    assert_eq!(receipt.metrics.stage_elapsed_micros.ripgrep, 41);
    assert_eq!(receipt.evidence.resident_graph.state, "skipped");
    assert_eq!(receipt.decision.chosen_path, "cold-rg-native-syntax-fusion");
}

#[test]
fn cold_rg_receipt_rejects_cross_generation_or_dual_lexical_execution() {
    let mut cross_generation = input();
    cross_generation.indexed_lexical_executed = false;
    cross_generation.resident_graph_executed = false;
    cross_generation.cold_rg = Some(SearchPlaybookColdRgExecution {
        generation_digest: digest('f'),
        coverage_input_digest: digest('a'),
        candidate_owner_ids: vec!["src/lib.rs".to_owned()],
        elapsed_micros: 41,
        process_count: 1,
    });
    assert_eq!(
        build_search_playbook_receipt(cross_generation)
            .expect_err("cross-generation cold rg must fail closed"),
        "search playbook ripgrep evidence is invalid"
    );

    let mut dual = input();
    dual.cold_rg = Some(SearchPlaybookColdRgExecution {
        generation_digest: digest('1'),
        coverage_input_digest: digest('a'),
        candidate_owner_ids: vec!["src/lib.rs".to_owned()],
        elapsed_micros: 41,
        process_count: 1,
    });
    assert_eq!(
        build_search_playbook_receipt(dual)
            .expect_err("cold rg and Tantivy cannot execute in one request"),
        "search playbook cannot execute cold rg and Tantivy in one request"
    );
}

#[test]
fn resident_byte_read_is_not_mislabeled_as_tantivy_or_ripgrep_execution() {
    let mut input = input();
    input.indexed_lexical_executed = false;
    input.byte_evidence_executed = true;
    let receipt = build_search_playbook_receipt(input).expect("byte-evidence playbook receipt");
    assert_eq!(receipt.evidence.indexed_lexical.state, "skipped");
    assert!(
        receipt
            .evidence
            .indexed_lexical
            .candidate_owner_ids
            .is_empty()
    );
    assert_eq!(receipt.evidence.byte_evidence.state, "executed");
    assert_eq!(receipt.metrics.stage_elapsed_micros.indexed_lexical, 0);
    assert_eq!(receipt.metrics.stage_elapsed_micros.byte_evidence, 7);
    assert_eq!(receipt.metrics.stage_elapsed_micros.ripgrep, 0);
}

#[test]
fn relationship_receipt_binds_executed_python_graph_to_its_exact_generation() {
    let mut input = input();
    input.intent = "relationship".to_owned();
    input.python_graph = Some(SearchPlaybookPythonGraphExecution {
        generation_digest: digest('1'),
        projection_digest: digest('3'),
        candidate_owner_ids: vec!["src/runtime.rs".to_owned()],
        elapsed_micros: 13,
    });
    let receipt = build_search_playbook_receipt(input).expect("relationship playbook receipt");
    assert_eq!(receipt.evidence.python_graph.state, "executed");
    assert_eq!(receipt.evidence.python_graph.generation_digest, digest('1'));
    assert_eq!(receipt.metrics.stage_elapsed_micros.python_graph, 13);
    assert_eq!(receipt.evidence.correlation.candidate_union_count, 2);
}

#[test]
fn relationship_receipt_rejects_cross_generation_python_graph_evidence() {
    let mut input = input();
    input.intent = "relationship".to_owned();
    input.python_graph = Some(SearchPlaybookPythonGraphExecution {
        generation_digest: digest('f'),
        projection_digest: digest('3'),
        candidate_owner_ids: vec!["src/runtime.rs".to_owned()],
        elapsed_micros: 13,
    });

    let error = build_search_playbook_receipt(input)
        .expect_err("cross-generation Python Graph evidence must fail closed");
    assert_eq!(error, "search playbook Python Graph evidence is invalid");
}

#[test]
fn ready_native_syntax_rejects_owner_only_evidence() {
    let mut input = input();
    input.native_syntax_projections.clear();
    let error = build_search_playbook_receipt(input).expect_err("owner-only must fail closed");
    assert_eq!(
        error,
        "search playbook native syntax attachment state is invalid"
    );
}
