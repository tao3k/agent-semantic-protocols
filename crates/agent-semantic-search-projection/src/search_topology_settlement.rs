// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime admission for a Search projection of the reusable Project Topology.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use orgize::ast::{
    OrgSourceBlock, OrgSourceBlockDocument, OrgSourceBlockHeader, OrgSourceBlockHeaderValue,
};
use serde_json::Value;

use crate::search_topology_settlement_support::{
    digest_json, gql_alias, is_blake3_digest, org_render_error, quoted, render_edge, render_node,
    render_result_node, require_text_eq, required_array, required_object, required_object_field,
    required_text, required_u64, validate_projection,
};

pub const SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-topology-settlement";
pub const SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_VERSION: &str = "1";
/// Fixed public evidence bound for one Search Playbook result.
pub const WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT: usize = 30;

/// One immutable Search projection over an admitted Project Topology generation.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchTopologySettlement {
    packet: Value,
    derived_relation_count: u64,
}

impl SearchTopologySettlement {
    /// Join one executed Search acquisition result to the immutable Project
    /// Topology and admit the resulting request-local settlement.
    pub fn from_workspace_result(
        request_id: &str,
        workspace_result: &Value,
        library: &agent_semantic_topology::ProjectTopologyLibrary,
    ) -> Result<Self, SearchTopologySettlementError> {
        let result = required_object(workspace_result, "workspace Search result")?;
        require_text_eq(
            result,
            "schemaId",
            "agent.semantic-protocols.workspace-search-playbook-result",
        )?;
        require_text_eq(result, "schemaVersion", "1")?;
        let acquisition_incomplete = match required_text(result, "result")? {
            "exact-selector-ready"
            | "disambiguation-required"
            | "relationship-supported"
            | "no-match" => false,
            "refinement-required" => true,
            "contradiction" => {
                return invalid(
                    "search-contradiction",
                    "contradictory Search evidence cannot publish a Ready topology settlement",
                );
            }
            "provider-contract-failure" => {
                return invalid(
                    "search-provider-contract-failure",
                    "provider contract failure cannot publish a Ready topology settlement",
                );
            }
            _ => {
                return invalid(
                    "schema-invalid",
                    "workspace Search result has an unsupported result state",
                );
            }
        };
        let evidence = required_array(result, "evidence")?;
        let evidence_item_limit = usize::try_from(required_u64(result, "evidenceItemLimit")?)
            .map_err(|_| error("schema-invalid", "evidenceItemLimit exceeds usize"))?;
        if evidence_item_limit != WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT
            || evidence.len() > evidence_item_limit
        {
            return invalid(
                "search-evidence-bound-exceeded",
                "Search settlement accepts at most thirty ranked evidence nodes",
            );
        }

        let library_packet = required_object(library.as_json(), "Project Topology library")?;
        let library_nodes = required_array(library_packet, "nodes")?;
        let library_edges = required_array(library_packet, "edges")?;
        let mut rank_by_selector = BTreeMap::<String, u64>::new();
        let mut hit_by_selector = BTreeMap::<String, Value>::new();
        let mut selected_ids = BTreeSet::<String>::new();
        for (rank, item) in evidence.iter().enumerate() {
            let item = required_object(item, "evidence[]")?;
            let selector = required_text(item, "selector")?;
            if rank_by_selector
                .insert(selector.to_owned(), rank as u64 + 1)
                .is_some()
            {
                return invalid(
                    "duplicate-search-selector",
                    format!("Search evidence repeats selector {selector}"),
                );
            }
            let hit = required_object_field(item, "hit")?;
            if hit.is_empty() {
                return invalid(
                    "search-hit-evidence-empty",
                    format!("Search evidence has no structured hit values: {selector}"),
                );
            }
            hit_by_selector.insert(selector.to_owned(), Value::Object(hit.clone()));
            let node_id = library_nodes
                .iter()
                .filter_map(Value::as_object)
                .find(|node| node.get("selector").and_then(Value::as_str) == Some(selector))
                .and_then(|node| node.get("id"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    error(
                        "search-selector-absent-from-topology",
                        format!("Search selector is absent from Project Topology: {selector}"),
                    )
                })?;
            selected_ids.insert(node_id.to_owned());
        }

        // Add a bounded one-hop topology neighborhood so the Search result is
        // a relationship graph rather than a list of independently rendered hits.
        const TOPOLOGY_NODE_LIMIT: usize = 90;
        if !selected_ids.is_empty() {
            for edge in library_edges {
                let edge = required_object(edge, "Project Topology edges[]")?;
                let from = required_text(edge, "from")?;
                let to = required_text(edge, "to")?;
                if selected_ids.contains(from) || selected_ids.contains(to) {
                    if selected_ids.len() < TOPOLOGY_NODE_LIMIT {
                        selected_ids.insert(from.to_owned());
                    }
                    if selected_ids.len() < TOPOLOGY_NODE_LIMIT {
                        selected_ids.insert(to.to_owned());
                    }
                }
            }
        }
        let library_frontiers = library_packet
            .get("frontiers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for frontier in &library_frontiers {
            let frontier = required_object(frontier, "Project Topology frontiers[]")?;
            let anchor = required_text(frontier, "anchor")?;
            if selected_ids.contains(anchor) {
                selected_ids.insert(required_text(frontier, "target")?.to_owned());
            }
        }

        let mut nodes = Vec::new();
        for node in library_nodes {
            let node = required_object(node, "Project Topology nodes[]")?;
            let id = required_text(node, "id")?;
            if !selected_ids.contains(id) {
                continue;
            }
            let mut projected = serde_json::Map::new();
            for field in [
                "id",
                "language",
                "kind",
                "name",
                "selector",
                "ownerLocator",
                "annotation",
            ] {
                if let Some(value) = node.get(field) {
                    projected.insert(field.to_owned(), value.clone());
                }
            }
            if let Some(selector) = node.get("selector").and_then(Value::as_str)
                && let Some(rank) = rank_by_selector.get(selector).copied()
            {
                projected.insert(
                    "projection".to_owned(),
                    serde_json::json!({
                        "rank": rank,
                        "depth": 0,
                        "hit": hit_by_selector[selector].clone(),
                    }),
                );
            }
            nodes.push(Value::Object(projected));
        }

        let mut edges = Vec::new();
        for edge in library_edges {
            let edge = required_object(edge, "Project Topology edges[]")?;
            let from = required_text(edge, "from")?;
            let to = required_text(edge, "to")?;
            if !selected_ids.contains(from) || !selected_ids.contains(to) {
                continue;
            }
            let modality = required_text(edge, "modality")?;
            let (producer_authority, evidence_authority) = match modality {
                "parser-direct" => ("provider-parser", "provider-witness"),
                "declared" => ("source-contract", "contract-witness"),
                "derived" => ("project-topology-inference.v1", "proof-dag"),
                "proposed" => ("model-proposal", "model-premises"),
                _ => {
                    return invalid(
                        "unsupported-edge-modality",
                        format!("unsupported Project Topology modality {modality}"),
                    );
                }
            };
            let mut projected = serde_json::json!({
                "from": from,
                "to": to,
                "relation": required_text(edge, "relation")?,
                "modality": modality,
                "producerAuthority": producer_authority,
                "evidenceAuthority": evidence_authority,
                "witnesses": edge.get("witnesses").cloned().ok_or_else(|| {
                    error("schema-invalid", "Project Topology edge witnesses are absent")
                })?,
            });
            if modality == "derived" {
                projected["derivedBy"] = Value::String("project-topology-inference.v1".to_owned());
                projected["proofRef"] = Value::String(required_text(edge, "proofRef")?.to_owned());
            }
            edges.push(projected);
        }

        let mut selectors = rank_by_selector.keys().cloned().collect::<Vec<_>>();
        selectors.sort();
        let derived_relation_count = edges
            .iter()
            .filter(|edge| edge.get("modality").and_then(Value::as_str) == Some("derived"))
            .count() as u64;
        let edge_set_digest = digest_json(&Value::Array(edges.clone()))?;
        let evidence_binding_digest = digest_json(&Value::Array(evidence.to_vec()))?;
        let decision_core_digest = digest_json(&serde_json::json!({
            "requestId": request_id,
            "selectors": selectors,
        }))?;
        let frontiers = library_frontiers
            .into_iter()
            .filter(|frontier| {
                frontier.as_object().is_some_and(|frontier| {
                    frontier
                        .get("anchor")
                        .and_then(Value::as_str)
                        .is_some_and(|anchor| selected_ids.contains(anchor))
                        && frontier
                            .get("target")
                            .and_then(Value::as_str)
                            .is_some_and(|target| selected_ids.contains(target))
                })
            })
            .collect::<Vec<_>>();
        let frontier_coverage_refs = frontiers
            .iter()
            .filter_map(|frontier| frontier.get("coverageRef").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        let coverage_certificates = library_packet
            .get("coverageCertificates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|certificate| {
                certificate
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| frontier_coverage_refs.contains(id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let topology_delta_digest = digest_json(&serde_json::json!({
            "nodes": nodes,
            "edges": edges,
            "coverageCertificates": coverage_certificates,
            "frontiers": frontiers,
        }))?;
        let closure = required_object_field(library_packet, "closure")?;
        require_text_eq(closure, "state", "stable")?;
        let closure_receipt_digest = required_text(closure, "digest")?;
        let queryable = !acquisition_incomplete && !selectors.is_empty();
        let next_relation_set_digest = if acquisition_incomplete {
            digest_json(&serde_json::json!({
                "candidateRelationSetDigest": edge_set_digest,
                "continuation": "search-acquisition-incomplete",
            }))?
        } else {
            edge_set_digest.clone()
        };
        let mut inference = serde_json::json!({
            "programId": "project-topology-inference.v1",
            "engineProfile": "attached-project-topology.v1",
            "scope": "bounded",
            "state": if acquisition_incomplete { "incomplete" } else { "complete" },
            "terminationKind": if acquisition_incomplete { "budget-exhausted" } else { "fixed-point" },
            // No request-local inference runs here. The fixed point and proof
            // DAG are inherited from the admitted library closure.
            "iterationCount": 0,
            "traversalDepth": if queryable { 1 } else { 0 },
            "derivedRelationCount": derived_relation_count,
            "proofDagDigest": required_text(closure, "proofDagDigest")?,
            "proofArtifactLocator": format!("project-topology/{}/proof-dag", library.generation_digest()),
            "rankingReceiptDigest": decision_core_digest,
            "candidateClosureReceiptDigest": closure_receipt_digest,
            "candidateRelationSetDigest": edge_set_digest,
            "nextRelationSetDigest": next_relation_set_digest,
            "postRankingCertified": !acquisition_incomplete,
        });
        if acquisition_incomplete {
            inference["reasonKind"] = Value::String("search-acquisition-incomplete".to_owned());
        }
        let settlement = serde_json::json!({
            "schemaId": SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_ID,
            "schemaVersion": SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_VERSION,
            "protocolId": "agent.semantic-protocols.search-playbook",
            "protocolVersion": "1",
            "resultState": if acquisition_incomplete { "incomplete" } else if queryable { "queryable" } else { "empty" },
            "binding": {
                "projectWorkspaceIdentity": library.project_workspace_identity(),
                "sourceGenerationDigest": library.source_generation_digest(),
                "providerCatalogDigest": library.provider_catalog_digest(),
                "topologyLibraryDigest": library.library_digest(),
                "topologyGenerationDigest": library.generation_digest(),
                "structuralTopologyDigest": library.structural_topology_digest(),
                "semanticTopologyDigest": library.semantic_topology_digest(),
                "inferenceProgramDigest": library.inference_program_digest(),
                "topologyClosureDigest": library.closure_digest(),
                "evidenceBindingDigest": evidence_binding_digest,
                "decisionCoreDigest": decision_core_digest,
                "topologyDeltaDigest": topology_delta_digest,
            },
            "inference": inference,
            "nodes": nodes,
            "edges": edges,
            "coverageCertificates": coverage_certificates,
            "frontiers": frontiers,
            "rendering": {
                "format": "org-gql",
                "gqlBlockCount": 1,
                "ascentSourceExposed": false,
            },
            "terminal": if acquisition_incomplete {
                serde_json::json!({"state": "incomplete", "terminalCount": 1, "reasonKind": "search-acquisition-incomplete"})
            } else {
                serde_json::json!({"state": "ready", "terminalCount": 1})
            },
        });
        Self::admit_for_library(settlement, library)
    }

    /// Jointly admit a Search slice with the exact reusable topology generation.
    pub fn admit_for_library(
        packet: Value,
        library: &agent_semantic_topology::ProjectTopologyLibrary,
    ) -> Result<Self, SearchTopologySettlementError> {
        let settlement = Self::admit(packet)?;
        let binding = settlement
            .packet
            .get("binding")
            .and_then(Value::as_object)
            .expect("admitted settlement binding");
        let expected = [
            (
                "projectWorkspaceIdentity",
                library.project_workspace_identity(),
            ),
            ("sourceGenerationDigest", library.source_generation_digest()),
            ("providerCatalogDigest", library.provider_catalog_digest()),
            ("topologyLibraryDigest", library.library_digest()),
            ("topologyGenerationDigest", library.generation_digest()),
            (
                "structuralTopologyDigest",
                library.structural_topology_digest(),
            ),
            ("semanticTopologyDigest", library.semantic_topology_digest()),
            ("inferenceProgramDigest", library.inference_program_digest()),
            ("topologyClosureDigest", library.closure_digest()),
        ];
        if expected
            .iter()
            .any(|(field, expected)| binding.get(*field).and_then(Value::as_str) != Some(*expected))
        {
            return invalid(
                "topology-library-binding-mismatch",
                "Search settlement identity tuple does not match the admitted Project Topology",
            );
        }
        Ok(settlement)
    }

    /// Admit a settlement before it can be rendered or consumed by Query PlayBook.
    pub fn admit(packet: Value) -> Result<Self, SearchTopologySettlementError> {
        let object = required_object(&packet, "packet")?;
        require_text_eq(object, "schemaId", SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_ID)?;
        require_text_eq(
            object,
            "schemaVersion",
            SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_VERSION,
        )?;
        require_text_eq(
            object,
            "protocolId",
            "agent.semantic-protocols.search-playbook",
        )?;
        require_text_eq(object, "protocolVersion", "1")?;
        if object.contains_key("materializationSet") {
            return invalid(
                "search-materialization-set-removed",
                "V1 exposes canonical selectors only on their owning GQL nodes",
            );
        }
        let result_state = required_text(object, "resultState")?;

        let binding = required_object_field(object, "binding")?;
        for field in [
            "projectWorkspaceIdentity",
            "sourceGenerationDigest",
            "providerCatalogDigest",
            "topologyLibraryDigest",
            "topologyGenerationDigest",
            "structuralTopologyDigest",
            "semanticTopologyDigest",
            "inferenceProgramDigest",
            "topologyClosureDigest",
            "evidenceBindingDigest",
            "decisionCoreDigest",
            "topologyDeltaDigest",
        ] {
            let value = required_text(binding, field)?;
            if field != "projectWorkspaceIdentity" && !is_blake3_digest(value) {
                return invalid(
                    "schema-invalid",
                    format!("binding.{field} must be a blake3-256 digest"),
                );
            }
        }
        let semantic_topology_digest = required_text(binding, "semanticTopologyDigest")?;
        let inference = required_object_field(object, "inference")?;
        require_text_eq(inference, "programId", "project-topology-inference.v1")?;
        let inference_state = required_text(inference, "state")?;
        let termination_kind = required_text(inference, "terminationKind")?;
        let candidate_relation_set_digest = required_text(inference, "candidateRelationSetDigest")?;
        let next_relation_set_digest = required_text(inference, "nextRelationSetDigest")?;
        if required_text(inference, "candidateClosureReceiptDigest")?
            != required_text(binding, "topologyClosureDigest")?
        {
            return invalid(
                "topology-closure-receipt-mismatch",
                "Search inference must cite the admitted Project Topology closure receipt",
            );
        }
        if !is_blake3_digest(candidate_relation_set_digest)
            || !is_blake3_digest(next_relation_set_digest)
        {
            return invalid(
                "schema-invalid",
                "fixed-point relation-set identities must be blake3-256 digests",
            );
        }
        let post_ranking_certified = inference
            .get("postRankingCertified")
            .and_then(Value::as_bool)
            .ok_or_else(|| error("schema-invalid", "postRankingCertified must be boolean"))?;

        let mut node_ids = BTreeSet::new();
        let mut selectors = BTreeSet::new();
        for node in required_array(object, "nodes")? {
            let node = required_object(node, "nodes[]")?;
            let node_id = required_text(node, "id")?;
            if !node_ids.insert(node_id.to_owned()) {
                return invalid("duplicate-node-id", format!("duplicate node id {node_id}"));
            }
            if let Some(selector) = node.get("selector").and_then(Value::as_str)
                && !selectors.insert(selector.to_owned())
            {
                return invalid(
                    "duplicate-selector",
                    format!("selector {selector} is owned by more than one settled node"),
                );
            }
            if let Some(projection) = node.get("projection") {
                validate_projection(required_object(projection, "nodes[].projection")?)?;
            }
            if let Some(annotation) = node.get("annotation") {
                let annotation = required_object(annotation, "nodes[].annotation")?;
                if required_text(annotation, "bindingDigest")? != semantic_topology_digest {
                    return invalid(
                        "annotation-binding-mismatch",
                        format!("annotation {node_id} is bound to another semantic topology"),
                    );
                }
            }
        }

        let authority_by_modality = BTreeMap::from([
            ("parser-direct", ("provider-parser", "provider-witness")),
            ("declared", ("source-contract", "contract-witness")),
            ("derived", ("project-topology-inference.v1", "proof-dag")),
            ("proposed", ("model-proposal", "model-premises")),
        ]);
        let mut derived_relation_count = 0_u64;
        for edge in required_array(object, "edges")? {
            let edge = required_object(edge, "edges[]")?;
            let from = required_text(edge, "from")?;
            let to = required_text(edge, "to")?;
            if !node_ids.contains(from) || !node_ids.contains(to) {
                return invalid("dangling-edge", format!("edge {from}->{to} is dangling"));
            }
            let modality = required_text(edge, "modality")?;
            let expected = authority_by_modality.get(modality).ok_or_else(|| {
                error(
                    "unsupported-edge-modality",
                    format!("unsupported modality {modality}"),
                )
            })?;
            if required_text(edge, "producerAuthority")? != expected.0
                || required_text(edge, "evidenceAuthority")? != expected.1
            {
                return invalid(
                    "edge-authority-modality-mismatch",
                    format!("edge {from}->{to} has incompatible authority"),
                );
            }
            if modality == "derived" {
                derived_relation_count += 1;
                required_text(edge, "proofRef")?;
            }
        }
        if required_u64(inference, "derivedRelationCount")? != derived_relation_count {
            return invalid(
                "derived-count-mismatch",
                "fixed-point derived count does not match admitted edges",
            );
        }

        let mut coverage = BTreeMap::new();
        for certificate in required_array(object, "coverageCertificates")? {
            let certificate = required_object(certificate, "coverageCertificates[]")?;
            let id = required_text(certificate, "id")?;
            if coverage.insert(id.to_owned(), certificate).is_some() {
                return invalid(
                    "duplicate-coverage-id",
                    format!("duplicate coverage id {id}"),
                );
            }
        }
        let mut referenced_coverage = BTreeSet::new();
        for frontier in required_array(object, "frontiers")? {
            let frontier = required_object(frontier, "frontiers[]")?;
            let anchor = required_text(frontier, "anchor")?;
            let target = required_text(frontier, "target")?;
            if !node_ids.contains(anchor) || !node_ids.contains(target) {
                return invalid(
                    "dangling-frontier",
                    format!("frontier {anchor}->{target} has an absent endpoint"),
                );
            }
            let state = required_text(frontier, "state")?;
            let reason = required_text(frontier, "reason")?;
            let coverage_ref = frontier.get("coverageRef").and_then(Value::as_str);
            let required_scope = match (state, reason, coverage_ref) {
                ("unknown", "binding-not-established", None) => continue,
                ("unknown", "coverage-open", Some(reference)) => {
                    referenced_coverage.insert(reference.to_owned());
                    "partial"
                }
                ("certified-missing", "complete-coverage-no-witness", Some(reference)) => {
                    referenced_coverage.insert(reference.to_owned());
                    "complete"
                }
                _ => {
                    return invalid(
                        "frontier-classification-mismatch",
                        format!(
                            "frontier {anchor}->{target} has an invalid state/reason/coverage tuple"
                        ),
                    );
                }
            };
            let coverage_ref = coverage_ref.expect("covered frontier has a reference");
            let certificate = coverage.get(coverage_ref).ok_or_else(|| {
                error(
                    "frontier-coverage-unresolved",
                    format!("coverage {coverage_ref} is absent"),
                )
            })?;
            if required_text(certificate, "scope")? != required_scope
                || required_text(certificate, "relation")? != required_text(frontier, "relation")?
                || required_text(certificate, "targetKind")?
                    != required_text(frontier, "targetKind")?
            {
                return invalid(
                    "frontier-coverage-incomplete",
                    format!("coverage {coverage_ref} does not classify this frontier"),
                );
            }
        }
        if referenced_coverage != coverage.keys().cloned().collect::<BTreeSet<_>>() {
            return invalid(
                "frontier-coverage-extraneous",
                "coverage certificates must equal exactly the references retained by frontiers",
            );
        }

        let rendering = required_object_field(object, "rendering")?;
        require_text_eq(rendering, "format", "org-gql")?;
        if required_u64(rendering, "gqlBlockCount")? != 1
            || rendering
                .get("ascentSourceExposed")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return invalid(
                "rendering-contract-mismatch",
                "settlement must expose exactly one GQL block and no Ascent source",
            );
        }
        let terminal = required_object_field(object, "terminal")?;
        if required_u64(terminal, "terminalCount")? != 1 {
            return invalid(
                "terminal-count-mismatch",
                "settlement must have one terminal",
            );
        }
        let terminal_state = required_text(terminal, "state")?;
        match termination_kind {
            "fixed-point" => {
                if inference_state != "complete" || !post_ranking_certified {
                    return invalid(
                        "inference-terminal-mismatch",
                        "fixed-point inference must be complete and post-ranking certified",
                    );
                }
                if candidate_relation_set_digest != next_relation_set_digest {
                    return invalid(
                        "fixed-point-not-reached",
                        "candidate and next relation sets differ",
                    );
                }
                if terminal_state != "ready" {
                    return invalid(
                        "inference-terminal-mismatch",
                        "fixed-point inference requires a ready terminal",
                    );
                }
                let expected_result = if selectors.is_empty() {
                    "empty"
                } else {
                    "queryable"
                };
                if result_state != expected_result {
                    return invalid(
                        "result-state-mismatch",
                        "result state does not match selector-bearing GQL nodes",
                    );
                }
            }
            "budget-exhausted" => {
                if inference_state != "incomplete"
                    || post_ranking_certified
                    || result_state != "incomplete"
                    || terminal_state != "incomplete"
                {
                    return invalid(
                        "inference-terminal-mismatch",
                        "budget exhaustion must remain an incomplete Search settlement",
                    );
                }
                if candidate_relation_set_digest == next_relation_set_digest {
                    return invalid(
                        "budget-terminal-claims-fixed-point",
                        "budget exhaustion cannot report an already stable relation set",
                    );
                }
            }
            "blocked" => {
                if inference_state != "blocked"
                    || post_ranking_certified
                    || result_state != "blocked"
                    || terminal_state != "failed"
                {
                    return invalid(
                        "inference-terminal-mismatch",
                        "blocked inference must fail without a ready Search settlement",
                    );
                }
            }
            _ => return invalid("schema-invalid", "unsupported inference termination"),
        }
        if termination_kind != "fixed-point"
            && required_text(inference, "reasonKind")? != required_text(terminal, "reasonKind")?
        {
            return invalid(
                "inference-terminal-mismatch",
                "inference and terminal reason kinds differ",
            );
        }

        Ok(Self {
            packet,
            derived_relation_count,
        })
    }

    /// Number of canonical selectors exposed directly on the settled GQL nodes.
    pub fn queryable_selector_count(&self) -> usize {
        self.packet["nodes"].as_array().map_or(0, |nodes| {
            nodes
                .iter()
                .filter(|node| node.get("selector").is_some())
                .count()
        })
    }

    pub fn derived_relation_count(&self) -> u64 {
        self.derived_relation_count
    }

    pub fn as_json(&self) -> &Value {
        &self.packet
    }

    /// Render the admitted settlement as the single Agent-facing Org/GQL block.
    pub fn render_org_gql(&self) -> Result<String, SearchTopologySettlementError> {
        let packet = required_object(&self.packet, "packet")?;
        let mut lines = Vec::new();
        let mut result_nodes = BTreeMap::<String, Vec<String>>::new();
        let mut standalone_nodes = Vec::new();

        for node in required_array(packet, "nodes")? {
            let node = required_object(node, "nodes[]")?;
            if node.contains_key("annotation") || node.contains_key("excerpt") {
                standalone_nodes.push(render_node(node)?);
            } else {
                let (language, rendered) = render_result_node(node)?;
                result_nodes.entry(language).or_default().push(rendered);
            }
        }
        for (language, nodes) in result_nodes {
            lines.push(format!(
                "({}:Language)-[:RESULTS]->[{}]",
                gql_alias(&language),
                nodes.join(",")
            ));
        }
        lines.extend(standalone_nodes);
        for edge in required_array(packet, "edges")? {
            lines.push(render_edge(required_object(edge, "edges[]")?)?);
        }
        for certificate in required_array(packet, "coverageCertificates")? {
            let certificate = required_object(certificate, "coverageCertificates[]")?;
            lines.push(format!(
                "({}:CoverageCertificate {{relation:{},target_kind:{},scope:{},digest:{}}})",
                gql_alias(required_text(certificate, "id")?),
                quoted(required_text(certificate, "relation")?),
                quoted(required_text(certificate, "targetKind")?),
                quoted(required_text(certificate, "scope")?),
                quoted(required_text(certificate, "digest")?),
            ));
        }
        for frontier in required_array(packet, "frontiers")? {
            let frontier = required_object(frontier, "frontiers[]")?;
            let mut properties = vec![
                format!("relation:{}", quoted(required_text(frontier, "relation")?)),
                format!(
                    "target_kind:{}",
                    quoted(required_text(frontier, "targetKind")?)
                ),
                format!("depth:{}", required_u64(frontier, "depth")?),
                format!("state:{}", quoted(required_text(frontier, "state")?)),
                format!("reason:{}", quoted(required_text(frontier, "reason")?)),
            ];
            if let Some(reference) = frontier.get("coverageRef").and_then(Value::as_str) {
                properties.push(format!("coverage:{}", quoted(reference)));
            }
            lines.push(format!(
                "({})-[:FRONTIER {{{}}}]->({})",
                required_text(frontier, "anchor")?,
                properties.join(","),
                required_text(frontier, "target")?,
            ));
        }
        let block = OrgSourceBlock::new(
            "gql",
            vec![
                OrgSourceBlockHeader::new(
                    "name",
                    OrgSourceBlockHeaderValue::token("result").map_err(org_render_error)?,
                )
                .map_err(org_render_error)?,
            ],
            vec![],
            lines.join("\n"),
        )
        .map_err(org_render_error)?;
        OrgSourceBlockDocument::new(vec![block])
            .and_then(|document| document.render())
            .map_err(org_render_error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchTopologySettlementError {
    reason_kind: &'static str,
    message: String,
}

impl SearchTopologySettlementError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for SearchTopologySettlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for SearchTopologySettlementError {}

pub(super) fn error(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> SearchTopologySettlementError {
    SearchTopologySettlementError {
        reason_kind,
        message: message.into(),
    }
}

pub(super) fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, SearchTopologySettlementError> {
    Err(error(reason_kind, message))
}
