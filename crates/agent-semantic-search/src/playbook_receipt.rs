//! One public Search playbook receipt over an admitted resident generation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{NativeSyntaxProjection, NativeSyntaxRelation, ResidentGraphSearchStage};
use agent_semantic_search_projection::RuntimeProviderSearchReceipt;

pub const SEARCH_PLAYBOOK_RECEIPT_SCHEMA_ID: &str = "asp.search.playbook-receipt";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub language_id: String,
    pub provider_digest: String,
    pub index_artifact_digest: String,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub operation_id: String,
    pub candidate_count: usize,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: agent_semantic_search_projection::ResidentSearchWorkCounters,
    pub query: String,
    pub intent: String,
    pub state: String,
    pub plan: SearchPlaybookPlan,
    pub evidence: SearchPlaybookEvidence,
    pub decision: SearchPlaybookDecision,
    pub metrics: SearchPlaybookMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookPlan {
    pub stages: Vec<SearchPlaybookStage>,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookStage {
    pub family: String,
    pub capability_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookEvidence {
    pub source_acquisition: SearchPlaybookSourceAcquisitionEvidence,
    pub native_syntax: SearchPlaybookNativeSyntaxEvidence,
    pub indexed_lexical: SearchPlaybookLexicalEvidence,
    pub byte_evidence: SearchPlaybookByteEvidence,
    pub resident_graph: SearchPlaybookGraphEvidence,
    pub ripgrep: SearchPlaybookRgEvidence,
    pub python_graph: SearchPlaybookPythonGraphEvidence,
    pub correlation: SearchPlaybookCorrelation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookSourceAcquisitionEvidence {
    pub state: String,
    pub stage_artifact_digest: String,
    pub generation_digest: String,
    pub admitted_owner_count: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookNativeSyntaxEvidence {
    pub state: String,
    pub stage_artifact_digest: String,
    pub projections: Vec<NativeSyntaxProjection>,
    pub relations: Vec<NativeSyntaxRelation>,
    pub proper_byte_ranges: bool,
    pub non_empty_query_keys: bool,
    pub relations_bound_to_owners: bool,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookLexicalEvidence {
    pub state: String,
    pub backend: String,
    pub generation_digest: String,
    pub index_artifact_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub indexed_owner_count: usize,
    pub admitted_owner_count: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookGraphEvidence {
    pub state: String,
    pub backend: String,
    pub generation_digest: String,
    pub result_digest: String,
    pub entry_owner_ids: Vec<String>,
    pub entry_node_ids: Vec<String>,
    pub candidate_owner_ids: Vec<String>,
    pub closure_state: String,
    pub unresolved_frontier_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookByteEvidence {
    pub state: String,
    pub mode: String,
    pub generation_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub coverage_complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookRgEvidence {
    pub state: String,
    pub reason_kind: String,
    pub mode: String,
    pub generation_digest: String,
    pub coverage_input_digest: Option<String>,
    pub candidate_owner_ids: Vec<String>,
    pub process_count: u8,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookPythonGraphEvidence {
    pub state: String,
    pub reason_kind: String,
    pub backend: String,
    pub generation_digest: String,
    pub projection_digest: Option<String>,
    pub candidate_owner_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookCorrelation {
    pub lexical_graph_overlap_count: usize,
    pub lexical_graph_jaccard_permille: usize,
    pub graph_marginal_candidate_count: usize,
    pub candidate_union_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookDecision {
    pub chosen_path: String,
    pub explanation: String,
    pub residual_uncertainty: Vec<String>,
    pub next_command: Option<String>,
    pub selectors: Vec<String>,
    pub owner_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookMetrics {
    pub total_elapsed_micros: u64,
    pub stage_elapsed_micros: SearchPlaybookStageMetrics,
    pub command_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPlaybookStageMetrics {
    pub native_syntax: u64,
    pub indexed_lexical: u64,
    pub byte_evidence: u64,
    pub resident_graph: u64,
    pub ripgrep: u64,
    pub python_graph: u64,
}

pub struct SearchPlaybookReceiptInput {
    pub workspace_identity: String,
    pub query: String,
    pub intent: String,
    pub coverage: String,
    pub max_owners: u32,
    pub deadline_ms: u64,
    pub indexed_owner_count: usize,
    pub indexed_lexical_executed: bool,
    pub resident_graph_executed: bool,
    pub byte_evidence_executed: bool,
    pub source_acquisition_stage_artifact_digest: String,
    pub native_syntax_state: String,
    pub native_syntax_stage_artifact_digest: String,
    pub native_syntax_projections: Vec<NativeSyntaxProjection>,
    pub native_syntax_relations: Vec<NativeSyntaxRelation>,
    pub native_syntax_elapsed_micros: u64,
    pub runtime: RuntimeProviderSearchReceipt,
    pub graph: ResidentGraphSearchStage,
    pub cold_rg: Option<SearchPlaybookColdRgExecution>,
    pub python_graph: Option<SearchPlaybookPythonGraphExecution>,
}

pub struct SearchPlaybookColdRgExecution {
    pub generation_digest: String,
    pub coverage_input_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub elapsed_micros: u64,
    pub process_count: u8,
}

pub struct SearchPlaybookPythonGraphExecution {
    pub generation_digest: String,
    pub projection_digest: String,
    pub candidate_owner_ids: Vec<String>,
    pub elapsed_micros: u64,
}

pub fn build_search_playbook_receipt(
    input: SearchPlaybookReceiptInput,
) -> Result<SearchPlaybookReceipt, String> {
    let cold_rg_executed = input.cold_rg.is_some();
    if cold_rg_executed && input.indexed_lexical_executed {
        return Err("search playbook cannot execute cold rg and Tantivy in one request".to_owned());
    }
    let python_graph_executed = input.python_graph.is_some();
    let mut lexical_owners = input.runtime.owner_paths.clone();
    lexical_owners.sort();
    lexical_owners.dedup();
    let mut graph_owners = input.graph.ranked_owner_paths.clone();
    graph_owners.sort();
    graph_owners.dedup();
    let lexical = if input.indexed_lexical_executed || cold_rg_executed {
        lexical_owners.iter().cloned().collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    let graph = if input.resident_graph_executed {
        graph_owners.iter().cloned().collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    let python_graph = input
        .python_graph
        .as_ref()
        .map(|execution| {
            execution
                .candidate_owner_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let overlap = lexical.intersection(&graph).count();
    let mut candidate_union = lexical.union(&graph).cloned().collect::<BTreeSet<_>>();
    candidate_union.extend(python_graph.iter().cloned());
    let union = candidate_union.len();
    let graph_marginal = graph.difference(&lexical).count();
    let jaccard = if union == 0 {
        0
    } else {
        overlap.saturating_mul(1000) / union
    };
    let entry_node_ids = lexical_owners
        .iter()
        .map(|owner| crate::stable_graph_node_id("owner", owner))
        .collect();
    let generation_digest = input.runtime.generation_digest.clone();
    let index_artifact_digest = input.runtime.index_artifact_digest.clone();
    let complete_coverage_requested = input.coverage == "complete";
    let receipt = SearchPlaybookReceipt {
        schema_id: SEARCH_PLAYBOOK_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: input.workspace_identity,
        language_id: input.runtime.language_id.clone(),
        provider_digest: input.runtime.provider_digest.clone(),
        index_artifact_digest: input.runtime.index_artifact_digest.clone(),
        generation_digest: input.runtime.generation_digest.clone(),
        source_root_digest: input.runtime.root_digest.clone(),
        operation_id: input.runtime.operation_id.clone(),
        candidate_count: input.runtime.candidate_count,
        resident_read_elapsed_micros: input.runtime.resident_read_elapsed_micros,
        service_elapsed_micros: input.runtime.service_elapsed_micros,
        elapsed_micros: input.runtime.elapsed_micros,
        work_counters: input.runtime.work_counters.clone(),
        query: input.query,
        intent: input.intent,
        state: "completed".to_owned(),
        plan: SearchPlaybookPlan {
            stages: canonical_search_playbook_stages(),
            coverage: input.coverage,
            max_owners: input.max_owners,
            deadline_ms: input.deadline_ms,
        },
        evidence: SearchPlaybookEvidence {
            source_acquisition: SearchPlaybookSourceAcquisitionEvidence {
                state: "admitted".to_owned(),
                stage_artifact_digest: input.source_acquisition_stage_artifact_digest,
                generation_digest: generation_digest.clone(),
                admitted_owner_count: input.indexed_owner_count,
                complete: true,
            },
            native_syntax: SearchPlaybookNativeSyntaxEvidence {
                state: input.native_syntax_state.clone(),
                stage_artifact_digest: input.native_syntax_stage_artifact_digest,
                projections: input.native_syntax_projections,
                relations: input.native_syntax_relations,
                proper_byte_ranges: input.native_syntax_state == "ready",
                non_empty_query_keys: input.native_syntax_state == "ready",
                relations_bound_to_owners: input.native_syntax_state == "ready",
                complete: input.native_syntax_state == "ready",
            },
            indexed_lexical: SearchPlaybookLexicalEvidence {
                state: if input.indexed_lexical_executed {
                    "executed"
                } else {
                    "skipped"
                }
                .to_owned(),
                backend: "tantivy".to_owned(),
                generation_digest: generation_digest.clone(),
                index_artifact_digest: index_artifact_digest.clone(),
                candidate_owner_ids: if input.indexed_lexical_executed {
                    lexical.iter().cloned().collect()
                } else {
                    Vec::new()
                },
                indexed_owner_count: input.indexed_owner_count,
                admitted_owner_count: lexical.len(),
                complete: input.indexed_lexical_executed,
            },
            byte_evidence: SearchPlaybookByteEvidence {
                state: if input.byte_evidence_executed {
                    "executed"
                } else {
                    "skipped"
                }
                .to_owned(),
                mode: if complete_coverage_requested {
                    "prove-coverage"
                } else {
                    "verify-candidates"
                }
                .to_owned(),
                generation_digest: generation_digest.clone(),
                candidate_owner_ids: if input.byte_evidence_executed {
                    lexical_owners.clone()
                } else {
                    Vec::new()
                },
                coverage_complete: input.byte_evidence_executed && complete_coverage_requested,
            },
            resident_graph: SearchPlaybookGraphEvidence {
                state: if input.resident_graph_executed {
                    "executed"
                } else {
                    "skipped"
                }
                .to_owned(),
                backend: "rust-resident-graph".to_owned(),
                generation_digest: input.graph.generation_digest.clone(),
                result_digest: input.graph.result_digest.clone(),
                entry_owner_ids: if input.resident_graph_executed {
                    lexical_owners.clone()
                } else {
                    Vec::new()
                },
                entry_node_ids: if input.resident_graph_executed {
                    entry_node_ids
                } else {
                    Vec::new()
                },
                candidate_owner_ids: if input.resident_graph_executed {
                    graph_owners
                } else {
                    Vec::new()
                },
                closure_state: if input.resident_graph_executed {
                    "complete"
                } else {
                    "unavailable"
                }
                .to_owned(),
                unresolved_frontier_count: 0,
            },
            ripgrep: SearchPlaybookRgEvidence {
                state: if cold_rg_executed { "executed" } else { "skipped" }.to_owned(),
                reason_kind: if cold_rg_executed {
                    "content-generation-cold-recall"
                } else {
                    "ready-path-does-not-execute-ripgrep"
                }
                .to_owned(),
                mode: if cold_rg_executed {
                    "immutable-generation-corpus"
                } else {
                    "not-requested"
                }
                .to_owned(),
                generation_digest: input
                    .cold_rg
                    .as_ref()
                    .map_or_else(|| generation_digest.clone(), |execution| {
                        execution.generation_digest.clone()
                    }),
                coverage_input_digest: input
                    .cold_rg
                    .as_ref()
                    .map(|execution| execution.coverage_input_digest.clone()),
                candidate_owner_ids: input
                    .cold_rg
                    .as_ref()
                    .map(|execution| {
                        let mut owners = execution.candidate_owner_ids.clone();
                        owners.sort();
                        owners.dedup();
                        owners
                    })
                    .unwrap_or_default(),
                process_count: input
                    .cold_rg
                    .as_ref()
                    .map_or(0, |execution| execution.process_count),
                complete: cold_rg_executed,
            },
            python_graph: SearchPlaybookPythonGraphEvidence {
                state: if input.python_graph.is_some() {
                    "executed"
                } else {
                    "skipped"
                }
                .to_owned(),
                reason_kind: if input.python_graph.is_some() {
                    "exact-generation-resident-evaluation"
                } else {
                    "optional-capability-not-requested"
                }
                .to_owned(),
                backend: "asp-python-graphs".to_owned(),
                generation_digest: input
                    .python_graph
                    .as_ref()
                    .map_or_else(|| generation_digest.clone(), |execution| {
                        execution.generation_digest.clone()
                    }),
                projection_digest: input
                    .python_graph
                    .as_ref()
                    .map(|execution| execution.projection_digest.clone()),
                candidate_owner_ids: python_graph.into_iter().collect(),
            },
            correlation: SearchPlaybookCorrelation {
                lexical_graph_overlap_count: overlap,
                lexical_graph_jaccard_permille: jaccard,
                graph_marginal_candidate_count: graph_marginal,
                candidate_union_count: union,
            },
        },
        decision: SearchPlaybookDecision {
            chosen_path: if python_graph_executed {
                "resident-native-syntax-python-graph-fusion"
            } else if cold_rg_executed && input.resident_graph_executed {
                "cold-rg-native-syntax-graph-fusion"
            } else if cold_rg_executed {
                "cold-rg-native-syntax-fusion"
            } else if !input.resident_graph_executed {
                "resident-native-syntax-lexical"
            } else {
                "resident-native-syntax-fusion"
            }
            .to_owned(),
            explanation: if python_graph_executed {
                "One admitted generation correlated provider-native syntax, the executed Tantivy frontier, Rust resident-graph ranking, and an exact generation-bound ASP Python Graph evaluation; ripgrep remains explicitly skipped without fabricated evidence."
            } else if cold_rg_executed {
                "One admitted content generation executed one bounded ripgrep process over its immutable corpus, then projected provider-native syntax without waiting for Tantivy or graph attachments."
            } else {
                "One admitted generation correlated provider-native syntax with the actually executed Tantivy or resident byte-evidence read and Rust resident-graph ranking; ripgrep and Python Graph remain explicit skipped capabilities without fabricated evidence."
            }
            .to_owned(),
            residual_uncertainty: Vec::new(),
            next_command: None,
            selectors: input.runtime.selectors.clone(),
            owner_paths: input.runtime.owner_paths.clone(),
        },
        metrics: SearchPlaybookMetrics {
            total_elapsed_micros: input
                .runtime
                .elapsed_micros
                .saturating_add(input.native_syntax_elapsed_micros)
                .saturating_add(
                    input
                        .cold_rg
                        .as_ref()
                        .map_or(0, |execution| execution.elapsed_micros),
                ),
            stage_elapsed_micros: SearchPlaybookStageMetrics {
                native_syntax: input.native_syntax_elapsed_micros,
                indexed_lexical: if input.indexed_lexical_executed {
                    input.runtime.resident_read_elapsed_micros
                } else {
                    0
                },
                byte_evidence: if input.byte_evidence_executed {
                    input.runtime.resident_read_elapsed_micros
                } else {
                    0
                },
                resident_graph: if input.resident_graph_executed {
                    input.graph.elapsed_micros
                } else {
                    0
                },
                ripgrep: input
                    .cold_rg
                    .as_ref()
                    .map_or(0, |execution| execution.elapsed_micros),
                python_graph: input
                    .python_graph
                    .as_ref()
                    .map_or(0, |execution| execution.elapsed_micros),
            },
            command_count: 1,
        },
    };
    receipt.validate()?;
    Ok(receipt)
}

impl SearchPlaybookReceipt {
    pub fn validate(&self) -> Result<(), String> {
        let projection_owners = self
            .evidence
            .native_syntax
            .projections
            .iter()
            .map(|projection| projection.owner_path.as_str())
            .collect::<BTreeSet<_>>();
        let decision_owners = self
            .decision
            .owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if self.schema_id != SEARCH_PLAYBOOK_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.workspace_identity.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.operation_id.trim().is_empty()
            || self.elapsed_micros
                != self
                    .resident_read_elapsed_micros
                    .saturating_add(self.service_elapsed_micros)
            || self.query.trim().is_empty()
            || self.plan.stages != canonical_search_playbook_stages()
            || self.metrics.command_count != 1
            || crate::canonical_blake3_digest(&self.evidence.native_syntax.stage_artifact_digest)
                .as_deref()
                != Ok(self.evidence.native_syntax.stage_artifact_digest.as_str())
            || projection_owners.len() != self.evidence.native_syntax.projections.len()
            || self.evidence.source_acquisition.state != "admitted"
            || !self.evidence.source_acquisition.complete
            || self.evidence.source_acquisition.generation_digest != self.generation_digest
            || crate::canonical_blake3_digest(
                &self.evidence.source_acquisition.stage_artifact_digest,
            )
            .as_deref()
                != Ok(self
                    .evidence
                    .source_acquisition
                    .stage_artifact_digest
                    .as_str())
            || self.evidence.indexed_lexical.generation_digest != self.generation_digest
            || self.evidence.indexed_lexical.index_artifact_digest != self.index_artifact_digest
            || self.evidence.byte_evidence.generation_digest != self.generation_digest
            || self.evidence.resident_graph.backend != "rust-resident-graph"
            || self.evidence.resident_graph.generation_digest != self.generation_digest
            || crate::canonical_blake3_digest(&self.evidence.resident_graph.result_digest)
                .as_deref()
                != Ok(self.evidence.resident_graph.result_digest.as_str())
        {
            return Err("search playbook receipt is incomplete".to_owned());
        }
        match self.evidence.native_syntax.state.as_str() {
            "ready"
                if self.evidence.native_syntax.complete
                    && self.evidence.native_syntax.proper_byte_ranges
                    && self.evidence.native_syntax.non_empty_query_keys
                    && self.evidence.native_syntax.relations_bound_to_owners
                    && projection_owners == decision_owners => {}
            "queued" | "building" | "failed"
                if !self.evidence.native_syntax.complete
                    && !self.evidence.native_syntax.proper_byte_ranges
                    && !self.evidence.native_syntax.non_empty_query_keys
                    && !self.evidence.native_syntax.relations_bound_to_owners
                    && self.evidence.native_syntax.projections.is_empty()
                    && self.evidence.native_syntax.relations.is_empty() => {}
            _ => return Err("search playbook native syntax attachment state is invalid".to_owned()),
        }
        match self.evidence.resident_graph.state.as_str() {
            "executed"
                if self.evidence.resident_graph.closure_state == "complete"
                    && self.evidence.resident_graph.unresolved_frontier_count == 0 => {}
            "skipped"
                if self.evidence.resident_graph.closure_state == "unavailable"
                    && self.evidence.resident_graph.entry_owner_ids.is_empty()
                    && self.evidence.resident_graph.entry_node_ids.is_empty()
                    && self.evidence.resident_graph.candidate_owner_ids.is_empty() => {}
            _ => return Err("search playbook resident graph evidence is invalid".to_owned()),
        }
        match self.evidence.ripgrep.state.as_str() {
            "skipped"
                if self.evidence.ripgrep.reason_kind == "ready-path-does-not-execute-ripgrep"
                    && self.evidence.ripgrep.mode == "not-requested"
                    && self.evidence.ripgrep.generation_digest == self.generation_digest
                    && self.evidence.ripgrep.coverage_input_digest.is_none()
                    && self.evidence.ripgrep.candidate_owner_ids.is_empty()
                    && self.evidence.ripgrep.process_count == 0
                    && !self.evidence.ripgrep.complete => {}
            "executed"
                if self.evidence.ripgrep.reason_kind == "content-generation-cold-recall"
                    && self.evidence.ripgrep.mode == "immutable-generation-corpus"
                    && self.evidence.ripgrep.generation_digest == self.generation_digest
                    && self
                        .evidence
                        .ripgrep
                        .coverage_input_digest
                        .as_deref()
                        .is_some_and(|digest| {
                            crate::canonical_blake3_digest(digest).as_deref() == Ok(digest)
                        })
                    && self
                        .evidence
                        .ripgrep
                        .candidate_owner_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>()
                        == decision_owners
                    && self.evidence.ripgrep.process_count == 1
                    && self.evidence.ripgrep.complete => {}
            _ => return Err("search playbook ripgrep evidence is invalid".to_owned()),
        }
        match self.evidence.python_graph.state.as_str() {
            "skipped"
                if self.intent != "relationship"
                    && self.evidence.python_graph.reason_kind
                        == "optional-capability-not-requested"
                    && self.evidence.python_graph.generation_digest == self.generation_digest
                    && self.evidence.python_graph.projection_digest.is_none()
                    && self.evidence.python_graph.candidate_owner_ids.is_empty() => {}
            "executed"
                if self.intent == "relationship"
                    && self.evidence.python_graph.reason_kind
                        == "exact-generation-resident-evaluation"
                    && self.evidence.python_graph.generation_digest == self.generation_digest
                    && self
                        .evidence
                        .python_graph
                        .projection_digest
                        .as_deref()
                        .is_some_and(|digest| {
                            crate::canonical_blake3_digest(digest).as_deref() == Ok(digest)
                        })
                    && crate::canonical_blake3_digest(
                        &self.evidence.python_graph.generation_digest,
                    )
                    .as_deref()
                        == Ok(self.evidence.python_graph.generation_digest.as_str()) => {}
            _ => return Err("search playbook Python Graph evidence is invalid".to_owned()),
        }
        self.work_counters.validate_zero_io()?;
        for projection in &self.evidence.native_syntax.projections {
            if projection.owner_path.trim().is_empty()
                || projection.selectors.is_empty()
                || crate::canonical_blake3_digest(&projection.content_digest).as_deref()
                    != Ok(projection.content_digest.as_str())
            {
                return Err("search playbook native syntax projection is incomplete".to_owned());
            }
            let mut selectors = BTreeSet::new();
            for selector in &projection.selectors {
                if !selectors.insert(selector.selector.as_str())
                    || selector.selector.trim().is_empty()
                    || selector.byte_start >= selector.byte_end
                    || selector.query_keys.is_empty()
                    || selector.query_keys.iter().any(|key| key.trim().is_empty())
                    || crate::canonical_blake3_digest(&selector.derived_projection_digest)
                        .as_deref()
                        != Ok(selector.derived_projection_digest.as_str())
                {
                    return Err("search playbook native syntax selector is invalid".to_owned());
                }
            }
        }
        let mut relations = BTreeSet::new();
        for relation in &self.evidence.native_syntax.relations {
            if !projection_owners.contains(relation.owner_path.as_str())
                || !relations.insert((
                    relation.owner_path.as_str(),
                    relation.relation_digest.as_str(),
                ))
                || crate::canonical_blake3_digest(&relation.relation_digest).as_deref()
                    != Ok(relation.relation_digest.as_str())
            {
                return Err("search playbook native syntax relation is invalid".to_owned());
            }
        }
        Ok(())
    }
}

fn canonical_search_playbook_stages() -> Vec<SearchPlaybookStage> {
    [
        ("acquire", "search.source-byte-acquisition"),
        ("acquire", "search.cold-rg-recall"),
        ("syntax", "search.native-syntax-playbook"),
        ("acquire", "search.tantivy-lexical"),
        ("reason", "search.rust-resident-graph"),
        ("reason", "search.python-graph"),
    ]
    .into_iter()
    .map(|(family, capability_id)| SearchPlaybookStage {
        family: family.to_owned(),
        capability_id: capability_id.to_owned(),
    })
    .collect()
}
