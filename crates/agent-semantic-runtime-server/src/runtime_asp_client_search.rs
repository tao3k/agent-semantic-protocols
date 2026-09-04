use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::RUNTIME_CLIENT_DISPATCH_BUDGET;
use crate::runtime_asp_client::elapsed_micros;
use crate::runtime_asp_client::record_runtime_route_performance;
use crate::runtime_query_generation::RuntimeQueryGeneration;
use agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle;
use agent_semantic_client_protocol::AspClientSearchRequest;
use agent_semantic_client_server::AspClientDispatchError;

#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_search_route(
    workspace_id: &str,
    request_id: &str,
    params: AspClientSearchRequest,
    language_id: &str,
    provider_id: &str,
    generation: &RuntimeQueryGeneration,
    runtime_search_service: &RuntimeSearchServiceHandle,
    telemetry_sender: &agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBusSender,
    dispatch_started: tokio::time::Instant,
) -> Result<serde_json::Value, AspClientOperationError> {
    let language = agent_semantic_client_core::LanguageId::try_from(language_id)
        .map_err(|error| format!("decode language id: {error}"))?;
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: language.clone(),
        provider_id: provider_id.into(),
    };
    let intent = agent_semantic_search::ResidentSearchIntent::parse(&params.intent)
        .map_err(AspClientOperationError::Message)?;
    let mut fusion_capabilities = generation.fusion_capabilities();
    if intent == agent_semantic_search::ResidentSearchIntent::Relationship {
        // The optional Python worker is admitted lazily below under the
        // exact immutable generation request. Its successful typed receipt,
        // not the base generation receipt, proves this capability.
        fusion_capabilities.python_graph = true;
    }
    let execution_plan =
        agent_semantic_search::plan_resident_search_execution(intent, fusion_capabilities)
            .map_err(|message| {
                AspClientOperationError::Terminal(AspClientDispatchError {
                    reason_kind: "query-not-ready".to_owned(),
                    message,
                    details: Some(serde_json::json!({
                        "phase": "resident-fusion-plan",
                        "intent": intent.as_str(),
                    })),
                })
            })?;
    let resident_started = tokio::time::Instant::now();
    let owner_scope = params.scope.strip_prefix("owner:");
    let mut cold_rg = None;
    let lookup = if execution_plan.use_cold_rg_candidates {
        let remaining = std::time::Duration::from_millis(params.deadline_ms)
            .min(RUNTIME_CLIENT_DISPATCH_BUDGET)
            .saturating_sub(dispatch_started.elapsed());
        let cold = crate::runtime_cold_rg::execute_runtime_cold_rg(
            generation.resident().cold_rg_corpus(),
            &params.query,
            params.max_owners,
            remaining,
        )
        .await
        .map_err(|message| {
            AspClientOperationError::Terminal(AspClientDispatchError {
                reason_kind: "cold-rg-execution-failed".to_owned(),
                message,
                details: Some(serde_json::json!({
                    "phase": "immutable-generation-cold-rg",
                    "intent": intent.as_str(),
                })),
            })
        })?;
        let mut owner_paths = cold.candidate_owner_paths.clone();
        if let Some(owner_path) = owner_scope {
            owner_paths.retain(|candidate| candidate == owner_path);
        }
        let lookup = generation.resident().read_cold_rg_candidates(
            &params.query,
            &owner_paths,
            Some(&authority),
            params.max_owners,
        );
        cold_rg = Some(agent_semantic_search::SearchPlaybookColdRgExecution {
            generation_digest: cold.content_generation_digest,
            coverage_input_digest: cold.coverage_input_digest,
            candidate_owner_ids: owner_paths,
            elapsed_micros: cold.elapsed_micros,
            process_count: cold.process_count,
        });
        lookup
    } else {
        match (execution_plan.verify_rg_bytes, owner_scope) {
            (true, Some(owner_path)) => generation.resident().read_byte_evidence_for_owner_scope(
                &params.query,
                owner_path,
                Some(&authority),
                params.max_owners,
            ),
            (true, None) => generation.resident().read_byte_evidence(
                &params.query,
                Some(&authority),
                params.max_owners,
            ),
            (false, Some(owner_path)) => generation.resident().read_source_index_for_owner_scope(
                &params.query,
                owner_path,
                Some(&authority),
                params.max_owners,
            ),
            (false, None) => generation.resident().read_source_index(
                &params.query,
                Some(&authority),
                params.max_owners,
            ),
        }
    }
    .map_err(|message| {
        let reason_kind = if message.starts_with("query-not-ready:") {
            "query-not-ready"
        } else {
            "resident-search-read-failed"
        };
        AspClientOperationError::Terminal(AspClientDispatchError {
            reason_kind: reason_kind.to_owned(),
            message,
            details: Some(serde_json::json!({
                "phase": "resident-fused-read",
                "intent": intent.as_str(),
            })),
        })
    })?;
    let resident_read_elapsed_micros = elapsed_micros(resident_started);
    let graph_stage = if execution_plan.project_resident_graph {
        crate::runtime_search_graph::rank_resident_search_frontier(
            request_id,
            intent,
            &params.query,
            language_id,
            provider_id,
            generation.generation_digest(),
            generation.resident(),
            &lookup.hits,
        )
        .map_err(|error| {
            AspClientOperationError::Terminal(AspClientDispatchError {
                reason_kind: error.reason_kind.to_owned(),
                message: error.message,
                details: error.details,
            })
        })?
    } else {
        agent_semantic_search::ResidentGraphSearchStage {
            generation_digest: generation.generation_digest().to_owned(),
            result_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"asp.search.resident-graph.unavailable.v1").to_hex()
            ),
            ranked_owner_paths: Vec::new(),
            elapsed_micros: 0,
            work: agent_semantic_search::ResidentGraphSearchWork::default(),
        }
    };
    let python_graph = if execution_plan.require_python_graph {
        Some(
            crate::runtime_search_graph::evaluate_python_relationship_graph(
                request_id,
                language_id,
                &params.query,
                generation.resident(),
                &lookup.hits,
                &runtime_search_service,
            )
            .await
            .map_err(|error| {
                AspClientOperationError::Terminal(AspClientDispatchError {
                    reason_kind: error.reason_kind.to_owned(),
                    message: error.message,
                    details: error.details,
                })
            })?,
        )
    } else {
        None
    };
    let mut selector_owner_paths = agent_semantic_search::bounded_ranked_selector_owner_paths(
        &lookup.hits,
        execution_plan
            .project_resident_graph
            .then_some(&graph_stage),
    );
    if let Some(python_graph) = &python_graph {
        selector_owner_paths.extend(python_graph.candidate_owner_ids.iter().cloned());
        selector_owner_paths.sort();
        selector_owner_paths.dedup();
        selector_owner_paths.truncate(agent_semantic_search::RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT);
    }
    let parser_owned_selector_pairs =
        generation.parser_owned_callable_selector_pairs(&selector_owner_paths)?;
    // Native syntax accounting covers every selected owner,
    // not only owners that happen to expose a callable.
    // Callable pairs remain a compact response projection;
    // they are not the generation membership authority.
    let native_syntax_owner_paths = selector_owner_paths.clone();
    let native_syntax_started = tokio::time::Instant::now();
    let fused_generation = &generation
        .resident()
        .search_generation_authority()
        .content_search_generation;
    fused_generation.validate()?;
    let native_syntax_state = generation.native_syntax_state();
    let (native_syntax_projections, native_syntax_relations, native_syntax_diagnostics) =
        if native_syntax_state == "ready" {
            generation.native_syntax_playbook_projection(&native_syntax_owner_paths)?
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
    let native_syntax_stage_artifact_digest = if native_syntax_state == "ready" {
        agent_semantic_search::build_native_syntax_stage_with_diagnostics(
            fused_generation.identity().clone(),
            native_syntax_projections.clone(),
            native_syntax_relations.clone(),
            native_syntax_diagnostics.clone(),
        )?
        .artifact_digest
    } else {
        format!(
            "blake3-256:{}",
            blake3::hash(
                format!(
                    "native-syntax-generation-v1\0{}\0{native_syntax_state}",
                    fused_generation.content_generation_digest
                )
                .as_bytes()
            )
            .to_hex()
        )
    };
    let native_syntax_elapsed_micros = elapsed_micros(native_syntax_started);
    record_runtime_route_performance(
        &telemetry_sender,
        workspace_id,
        language_id,
        generation.generation_digest(),
        request_id,
        "search",
        "runtime-graph-rank",
        &graph_stage.result_digest,
        graph_stage.elapsed_micros,
    )?;
    record_runtime_route_performance(
        &telemetry_sender,
        workspace_id,
        language_id,
        generation.generation_digest(),
        request_id,
        "search",
        "runtime-source-index-read",
        &params.intent,
        resident_read_elapsed_micros,
    )?;
    let runtime_receipt = agent_semantic_search::build_runtime_provider_search_receipt_with_graph(
        request_id.to_owned(),
        language,
        vec![agent_semantic_search::RuntimeSearchSource::shared(
            "resident",
            std::sync::Arc::clone(&lookup),
        )],
        resident_read_elapsed_micros,
        parser_owned_selector_pairs,
        execution_plan
            .project_resident_graph
            .then_some(graph_stage.clone()),
    )
    .await?;
    let receipt = agent_semantic_search::build_search_playbook_receipt(
        agent_semantic_search::SearchPlaybookReceiptInput {
            workspace_identity: workspace_id.to_owned(),
            query: params.query,
            intent: params.intent,
            coverage: params.coverage,
            max_owners: params.max_owners,
            deadline_ms: params.deadline_ms,
            indexed_owner_count: generation.resident().indexed_owner_count(),
            indexed_lexical_executed: execution_plan.use_tantivy_candidates,
            byte_evidence_executed: execution_plan.verify_rg_bytes,
            cold_rg,
            resident_graph_executed: execution_plan.project_resident_graph,
            source_acquisition_stage_artifact_digest: fused_generation
                .acquisition
                .artifact_digest
                .clone(),
            native_syntax_state: native_syntax_state.to_owned(),
            native_syntax_stage_artifact_digest,
            native_syntax_projections,
            native_syntax_relations,
            native_syntax_diagnostics,
            native_syntax_elapsed_micros,
            runtime: runtime_receipt,
            graph: graph_stage,
            python_graph,
        },
    )?;
    Ok(serde_json::to_value(receipt).map_err(|error| format!("encode search receipt: {error}"))?)
}
