//! Typed composition of indexed lexical recall, Python graph reasoning, and
//! bounded ripgrep verification.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecallQueryIntent {
    ExactSelector,
    Conceptual,
    Relationship,
    ExactLiteral,
    AbsenceProof,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LexicalBackendKind {
    TursoFts,
    TantivyHotOverlay,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphClosureState {
    Complete,
    FrontierOpen,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecallCalibrationState {
    Calibrated,
    Uncalibrated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchRouteStageFamily {
    Direct,
    Acquire,
    Reason,
    Verify,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallRouteStage {
    pub family: SearchRouteStageFamily,
    pub capability_id: String,
}

impl RecallRouteStage {
    fn direct_selector() -> Self {
        Self::new(SearchRouteStageFamily::Direct, "search.direct-selector")
    }

    fn indexed_lexical() -> Self {
        Self::new(SearchRouteStageFamily::Acquire, "search.indexed-lexical")
    }

    fn python_graph() -> Self {
        Self::new(SearchRouteStageFamily::Reason, "search.python-graph")
    }

    fn ripgrep_verify_candidates() -> Self {
        Self::new(
            SearchRouteStageFamily::Verify,
            "search.ripgrep-verify-candidates",
        )
    }

    fn ripgrep_prove_coverage() -> Self {
        Self::new(
            SearchRouteStageFamily::Verify,
            "search.ripgrep-prove-coverage",
        )
    }

    fn new(family: SearchRouteStageFamily, capability_id: &str) -> Self {
        Self {
            family,
            capability_id: capability_id.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedLexicalObservation {
    pub backend: LexicalBackendKind,
    pub admitted_owner_count: usize,
    pub indexed_owner_count: usize,
    pub candidate_owner_ids: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonGraphObservation {
    pub closure_state: GraphClosureState,
    pub seed_owner_ids: BTreeSet<String>,
    pub candidate_owner_ids: BTreeSet<String>,
    pub unresolved_frontier_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RipgrepVerificationObservation {
    pub admitted_owner_count: usize,
    pub covered_owner_count: usize,
    pub verified_owner_ids: BTreeSet<String>,
    pub matched_owner_ids: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct RecallCompositionRequest {
    pub workspace_identity: String,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub intent: RecallQueryIntent,
    pub exact_selector_current: bool,
    pub calibration_state: RecallCalibrationState,
    pub lexical: Option<IndexedLexicalObservation>,
    pub graph: Option<PythonGraphObservation>,
    pub ripgrep: Option<RipgrepVerificationObservation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallSetCorrelation {
    pub lexical_graph_overlap_count: usize,
    pub lexical_verified_overlap_count: usize,
    pub graph_verified_overlap_count: usize,
    pub lexical_graph_jaccard_permille: u16,
    pub graph_marginal_candidate_count: usize,
    pub verified_union_candidate_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallCompositionDecision {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub intent: RecallQueryIntent,
    pub calibration_state: RecallCalibrationState,
    pub stages: Vec<RecallRouteStage>,
    pub correlation: RecallSetCorrelation,
    pub residual_uncertainty: Vec<String>,
}

pub fn plan_recall_composition(
    request: RecallCompositionRequest,
) -> Result<RecallCompositionDecision, String> {
    validate_identity("workspace identity", &request.workspace_identity)?;
    validate_identity("generation digest", &request.generation_digest)?;
    validate_identity("source root digest", &request.source_root_digest)?;
    validate_observations(&request)?;

    let stages = route_stages(&request)?;
    let correlation = correlation(&request);
    let residual_uncertainty = residual_uncertainty(&request, &stages);

    Ok(RecallCompositionDecision {
        schema_id: "agent.semantic-protocols.search-recall-composition".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: request.workspace_identity,
        generation_digest: request.generation_digest,
        source_root_digest: request.source_root_digest,
        intent: request.intent,
        calibration_state: request.calibration_state,
        stages,
        correlation,
        residual_uncertainty,
    })
}

fn route_stages(request: &RecallCompositionRequest) -> Result<Vec<RecallRouteStage>, String> {
    if request.intent == RecallQueryIntent::ExactSelector && request.exact_selector_current {
        return Ok(vec![RecallRouteStage::direct_selector()]);
    }

    let mut stages = Vec::new();
    match request.intent {
        RecallQueryIntent::Relationship => {
            push_if(
                &mut stages,
                request.graph.is_some(),
                RecallRouteStage::python_graph(),
            );
            push_if(
                &mut stages,
                request.lexical.is_some(),
                RecallRouteStage::indexed_lexical(),
            );
            push_if(
                &mut stages,
                request.ripgrep.is_some(),
                RecallRouteStage::ripgrep_verify_candidates(),
            );
        }
        RecallQueryIntent::ExactLiteral => {
            push_if(
                &mut stages,
                request.lexical.is_some(),
                RecallRouteStage::indexed_lexical(),
            );
            push_if(
                &mut stages,
                request.ripgrep.is_some(),
                RecallRouteStage::ripgrep_verify_candidates(),
            );
            push_if(
                &mut stages,
                request.graph.is_some(),
                RecallRouteStage::python_graph(),
            );
        }
        RecallQueryIntent::AbsenceProof => push_if(
            &mut stages,
            request.ripgrep.is_some(),
            RecallRouteStage::ripgrep_prove_coverage(),
        ),
        RecallQueryIntent::ExactSelector | RecallQueryIntent::Conceptual => {
            push_if(
                &mut stages,
                request.lexical.is_some(),
                RecallRouteStage::indexed_lexical(),
            );
            push_if(
                &mut stages,
                request.graph.is_some(),
                RecallRouteStage::python_graph(),
            );
            push_if(
                &mut stages,
                request.ripgrep.is_some(),
                RecallRouteStage::ripgrep_verify_candidates(),
            );
        }
    }
    if stages.is_empty() {
        return Err("no recall lane is available for the requested intent".to_owned());
    }
    Ok(stages)
}

fn push_if(stages: &mut Vec<RecallRouteStage>, condition: bool, stage: RecallRouteStage) {
    if condition {
        stages.push(stage);
    }
}

fn correlation(request: &RecallCompositionRequest) -> RecallSetCorrelation {
    let empty = BTreeSet::new();
    let lexical = request
        .lexical
        .as_ref()
        .map_or(&empty, |lane| &lane.candidate_owner_ids);
    let graph = request
        .graph
        .as_ref()
        .map_or(&empty, |lane| &lane.candidate_owner_ids);
    let verified = request
        .ripgrep
        .as_ref()
        .map_or(&empty, |lane| &lane.verified_owner_ids);
    let lexical_graph_overlap_count = lexical.intersection(graph).count();
    let union_count = lexical.union(graph).count();
    RecallSetCorrelation {
        lexical_graph_overlap_count,
        lexical_verified_overlap_count: lexical.intersection(verified).count(),
        graph_verified_overlap_count: graph.intersection(verified).count(),
        lexical_graph_jaccard_permille: if union_count == 0 {
            0
        } else {
            u16::try_from(lexical_graph_overlap_count.saturating_mul(1_000) / union_count)
                .unwrap_or(1_000)
        },
        graph_marginal_candidate_count: graph.difference(lexical).count(),
        verified_union_candidate_count: lexical
            .union(graph)
            .filter(|id| verified.contains(*id))
            .count(),
    }
}

fn residual_uncertainty(
    request: &RecallCompositionRequest,
    stages: &[RecallRouteStage],
) -> Vec<String> {
    if stages
        .iter()
        .any(|stage| stage.capability_id == "search.direct-selector")
    {
        return Vec::new();
    }
    let mut residual = BTreeSet::new();
    if request.calibration_state == RecallCalibrationState::Uncalibrated {
        residual.insert("uncalibrated-route-quality");
    }
    match &request.lexical {
        Some(lane) if lane.indexed_owner_count < lane.admitted_owner_count => {
            residual.insert("indexed-owner-coverage-incomplete");
        }
        None if request.intent != RecallQueryIntent::AbsenceProof => {
            residual.insert("indexed-lexical-unavailable");
        }
        _ => {}
    }
    match &request.graph {
        Some(lane) if lane.closure_state == GraphClosureState::FrontierOpen => {
            residual.insert("graph-frontier-open");
        }
        Some(lane) if lane.closure_state == GraphClosureState::Stale => {
            residual.insert("graph-generation-stale");
        }
        None if request.intent == RecallQueryIntent::Relationship => {
            residual.insert("python-graph-unavailable");
        }
        _ => {}
    }
    match &request.ripgrep {
        Some(lane)
            if stages
                .iter()
                .any(|stage| stage.capability_id == "search.ripgrep-prove-coverage")
                && lane.covered_owner_count != lane.admitted_owner_count =>
        {
            residual.insert("coverage-proof-incomplete");
        }
        None if request.intent == RecallQueryIntent::AbsenceProof => {
            residual.insert("coverage-proof-unavailable");
        }
        None if !stages
            .iter()
            .any(|stage| stage.capability_id == "search.direct-selector") =>
        {
            residual.insert("candidate-byte-verification-unavailable");
        }
        _ => {}
    }
    residual.into_iter().map(str::to_owned).collect()
}

fn validate_identity(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must be non-empty"))
    } else {
        Ok(())
    }
}

fn validate_observations(request: &RecallCompositionRequest) -> Result<(), String> {
    if let Some(lane) = &request.lexical
        && lane.indexed_owner_count > lane.admitted_owner_count
    {
        return Err("indexed owner count exceeds admitted owner count".to_owned());
    }
    if let Some(lane) = &request.ripgrep {
        if lane.covered_owner_count > lane.admitted_owner_count {
            return Err("covered owner count exceeds admitted owner count".to_owned());
        }
        if !lane.matched_owner_ids.is_subset(&lane.verified_owner_ids) {
            return Err("ripgrep matches must be a subset of verified owners".to_owned());
        }
    }
    Ok(())
}
