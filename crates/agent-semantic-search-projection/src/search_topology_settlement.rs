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
use serde_json::Map;
use serde_json::Value;

pub const SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-topology-settlement";
pub const SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_VERSION: &str = "1";

/// One immutable Search projection over an admitted Project Topology generation.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchTopologySettlement {
    packet: Value,
    materialization_request_id: String,
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
        let evidence = required_array(result, "evidence")?;
        if evidence.len() > 30 {
            return invalid(
                "search-evidence-bound-exceeded",
                "Search settlement accepts at most thirty ranked evidence nodes",
            );
        }

        let library_packet = required_object(library.as_json(), "Project Topology library")?;
        let library_nodes = required_array(library_packet, "nodes")?;
        let library_edges = required_array(library_packet, "edges")?;
        let mut rank_by_selector = BTreeMap::<String, u64>::new();
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

        let mut nodes = Vec::new();
        let mut next_context_rank = rank_by_selector.len() as u64 + 1;
        for node in library_nodes {
            let node = required_object(node, "Project Topology nodes[]")?;
            let id = required_text(node, "id")?;
            if !selected_ids.contains(id) {
                continue;
            }
            let mut projected = serde_json::Map::new();
            for field in ["id", "language", "kind", "name", "selector", "annotation"] {
                if let Some(value) = node.get(field) {
                    projected.insert(field.to_owned(), value.clone());
                }
            }
            if let Some(selector) = node.get("selector").and_then(Value::as_str) {
                let (rank, depth) = rank_by_selector.get(selector).copied().map_or_else(
                    || {
                        let rank = next_context_rank;
                        next_context_rank += 1;
                        (rank, 1)
                    },
                    |rank| (rank, 0),
                );
                projected.insert(
                    "projection".to_owned(),
                    serde_json::json!({"rank": rank, "depth": depth}),
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
        let proof_dependencies = edges
            .iter()
            .filter(|edge| edge.get("modality").and_then(Value::as_str) == Some("derived"))
            .filter_map(|edge| {
                edge.get("proofRef")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let derived_relation_count = proof_dependencies.len() as u64;
        let edge_set_digest = digest_json(&Value::Array(edges.clone()))?;
        let evidence_binding_digest = digest_json(&Value::Array(evidence.to_vec()))?;
        let decision_core_digest = digest_json(&serde_json::json!({
            "requestId": request_id,
            "selectors": selectors,
        }))?;
        let topology_delta_digest = digest_json(&serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        }))?;
        let materialization_digest = digest_json(&serde_json::json!({
            "requestId": request_id,
            "selectors": selectors,
            "proofDependencies": proof_dependencies,
        }))?;
        let closure = required_object_field(library_packet, "closure")?;
        let materializable = !selectors.is_empty();
        let settlement = serde_json::json!({
            "schemaId": SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_ID,
            "schemaVersion": SEARCH_TOPOLOGY_SETTLEMENT_SCHEMA_VERSION,
            "protocolId": "agent.semantic-protocols.search-playbook",
            "protocolVersion": "1",
            "resultState": if materializable { "materializable" } else { "empty" },
            "binding": {
                "projectWorkspaceIdentity": library.project_workspace_identity(),
                "sourceGenerationDigest": library.source_generation_digest(),
                "providerCatalogDigest": library.provider_catalog_digest(),
                "topologyLibraryDigest": library.library_digest(),
                "topologyGenerationDigest": library.generation_digest(),
                "structuralTopologyDigest": library.structural_topology_digest(),
                "semanticTopologyDigest": library.semantic_topology_digest(),
                "inferenceProgramDigest": library.inference_program_digest(),
                "evidenceBindingDigest": evidence_binding_digest,
                "decisionCoreDigest": decision_core_digest,
                "topologyDeltaDigest": topology_delta_digest,
            },
            "inference": {
                "programId": "project-topology-inference.v1",
                "engineProfile": "attached-project-topology.v1",
                "scope": "bounded",
                "state": "complete",
                "terminationKind": "fixed-point",
                "iterationCount": 0,
                "traversalDepth": if materializable { 1 } else { 0 },
                "derivedRelationCount": derived_relation_count,
                "proofDagDigest": required_text(closure, "proofDagDigest")?,
                "proofArtifactLocator": format!("project-topology/{}/proof-dag", library.generation_digest()),
                "rankingReceiptDigest": decision_core_digest,
                "candidateClosureReceiptDigest": topology_delta_digest,
                "candidateRelationSetDigest": edge_set_digest,
                "nextRelationSetDigest": edge_set_digest,
                "postRankingCertified": true,
            },
            "nodes": nodes,
            "edges": edges,
            "coverageCertificates": library_packet.get("coverageCertificates").cloned().unwrap_or_else(|| Value::Array(vec![])),
            "frontiers": [],
            "materializationSet": {
                "state": if materializable { "available" } else { "empty" },
                "requestId": request_id,
                "digest": materialization_digest,
                "selectors": selectors,
                "proofDependencies": proof_dependencies,
            },
            "rendering": {
                "format": "org-gql",
                "gqlBlockCount": 1,
                "ascentSourceExposed": false,
            },
            "terminal": {"state": "ready", "terminalCount": 1},
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
            if let Some(selector) = node.get("selector").and_then(Value::as_str) {
                selectors.insert(selector.to_owned());
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
        let mut proof_refs = BTreeSet::new();
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
                proof_refs.insert(required_text(edge, "proofRef")?.to_owned());
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
        for frontier in required_array(object, "frontiers")? {
            let frontier = required_object(frontier, "frontiers[]")?;
            let anchor = required_text(frontier, "anchor")?;
            if !node_ids.contains(anchor) {
                return invalid(
                    "dangling-frontier",
                    format!("frontier anchor {anchor} is absent"),
                );
            }
            if required_text(frontier, "state")? == "certified-missing" {
                let coverage_ref = required_text(frontier, "coverageRef")?;
                let certificate = coverage.get(coverage_ref).ok_or_else(|| {
                    error(
                        "frontier-coverage-unresolved",
                        format!("coverage {coverage_ref} is absent"),
                    )
                })?;
                if required_text(certificate, "scope")? != "complete"
                    || required_text(certificate, "relation")?
                        != required_text(frontier, "relation")?
                    || required_text(certificate, "targetKind")?
                        != required_text(frontier, "targetKind")?
                {
                    return invalid(
                        "frontier-coverage-incomplete",
                        format!("coverage {coverage_ref} does not certify this frontier"),
                    );
                }
            }
        }

        let materialization = required_object_field(object, "materializationSet")?;
        let materialization_state = required_text(materialization, "state")?;
        let materialization_request_id = required_text(materialization, "requestId")?.to_owned();
        let selected = text_array(materialization, "selectors")?;
        if selected.windows(2).any(|pair| pair[0] >= pair[1]) {
            return invalid(
                "materialization-order",
                "materialization selectors must be strictly sorted and unique",
            );
        }
        if selected
            .iter()
            .any(|selector| !selectors.contains(*selector))
        {
            return invalid(
                "materialization-selector-unavailable",
                "materialization contains a selector absent from settled nodes",
            );
        }
        let selected_proofs = text_array(materialization, "proofDependencies")?;
        if selected_proofs
            .iter()
            .any(|proof| !proof_refs.contains(*proof))
        {
            return invalid(
                "proof-dependency-unresolved",
                "materialization contains an unresolved proof dependency",
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
                let expected_result = if materialization_state == "available" {
                    "materializable"
                } else {
                    "empty"
                };
                if result_state != expected_result {
                    return invalid(
                        "result-state-mismatch",
                        "result state does not match the materialization set",
                    );
                }
            }
            "budget-exhausted" => {
                if inference_state != "incomplete"
                    || post_ranking_certified
                    || result_state != "incomplete"
                    || terminal_state != "incomplete"
                    || materialization_state != "empty"
                    || !selected.is_empty()
                    || !selected_proofs.is_empty()
                {
                    return invalid(
                        "inference-terminal-mismatch",
                        "budget exhaustion must remain incomplete and non-materializable",
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
                    || materialization_state != "empty"
                    || !selected.is_empty()
                    || !selected_proofs.is_empty()
                {
                    return invalid(
                        "inference-terminal-mismatch",
                        "blocked inference must fail without materialization",
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
            materialization_request_id,
            derived_relation_count,
        })
    }

    pub fn materialization_request_id(&self) -> &str {
        &self.materialization_request_id
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

        for node in required_array(packet, "nodes")? {
            let node = required_object(node, "nodes[]")?;
            lines.push(render_node(node)?);
        }
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
        for (index, frontier) in required_array(packet, "frontiers")?.iter().enumerate() {
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
                "({})-[:FRONTIER {{{}}}]->(frontier_{index}:Unknown)",
                required_text(frontier, "anchor")?,
                properties.join(",")
            ));
        }
        let materialization = required_object_field(packet, "materializationSet")?;
        lines.push(format!(
            "(materialize:MaterializationSet {{request_id:{},digest:{},selectors:{},proofs:{}}})",
            quoted(required_text(materialization, "requestId")?),
            quoted(required_text(materialization, "digest")?),
            render_text_array(materialization, "selectors")?,
            render_text_array(materialization, "proofDependencies")?,
        ));
        let block = OrgSourceBlock::new(
            "gql",
            vec![
                OrgSourceBlockHeader::new(
                    "profile",
                    OrgSourceBlockHeaderValue::token("search-evidence.v1")
                        .map_err(org_render_error)?,
                )
                .map_err(org_render_error)?,
                OrgSourceBlockHeader::new(
                    "eval",
                    OrgSourceBlockHeaderValue::token("never").map_err(org_render_error)?,
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

fn digest_json(value: &Value) -> Result<String, SearchTopologySettlementError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|failure| error("rendering-failed", failure.to_string()))?;
    Ok(format!("blake3-256:{}", blake3::hash(&encoded).to_hex()))
}

fn org_render_error(
    error: orgize::ast::OrgSourceBlockDocumentError,
) -> SearchTopologySettlementError {
    SearchTopologySettlementError {
        reason_kind: "rendering-failed",
        message: error.to_string(),
    }
}

fn render_node(node: &Map<String, Value>) -> Result<String, SearchTopologySettlementError> {
    let id = required_text(node, "id")?;
    let language = required_text(node, "language")?;
    let kind = required_text(node, "kind")?;
    if let Some(annotation) = node.get("annotation") {
        let annotation = required_object(annotation, "nodes[].annotation")?;
        let mut properties = vec![
            format!("text:{}", quoted(required_text(annotation, "text")?)),
            format!("state:{}", quoted(required_text(annotation, "state")?)),
            format!(
                "binding:{}",
                quoted(required_text(annotation, "bindingDigest")?)
            ),
            format!(
                "premises:{}",
                render_text_array(annotation, "premiseWitnesses")?
            ),
            format!(
                "producer:{}",
                quoted(required_text(annotation, "producer")?)
            ),
        ];
        if let Some(reference) = annotation
            .get("admissionReceiptRef")
            .and_then(Value::as_str)
        {
            properties.push(format!("admission:{}", quoted(reference)));
        }
        return Ok(format!(
            "({id}:SemanticAnnotation {{{}}})",
            properties.join(",")
        ));
    }
    if let Some(excerpt) = node.get("excerpt") {
        let excerpt = required_object(excerpt, "nodes[].excerpt")?;
        return Ok(format!(
            "({id}:SourceHit {{language:{},path:{},match:{},read:{},witness:{}}})",
            quoted(language),
            quoted(required_text(excerpt, "path")?),
            serde_json::to_string(
                excerpt
                    .get("match")
                    .ok_or_else(|| error("schema-invalid", "excerpt match is absent"))?
            )
            .map_err(|failure| error("rendering-failed", failure.to_string()))?,
            serde_json::to_string(
                excerpt
                    .get("read")
                    .ok_or_else(|| error("schema-invalid", "excerpt read is absent"))?
            )
            .map_err(|failure| error("rendering-failed", failure.to_string()))?,
            quoted(required_text(excerpt, "witness")?),
        ));
    }

    let label = format!("{}{}", gql_type_prefix(language), gql_type_prefix(kind));
    let mut properties = Vec::new();
    if let Some(name) = node.get("name").and_then(Value::as_str) {
        properties.push(format!("name:{}", quoted(name)));
    }
    properties.push(format!(
        "selector:{}",
        quoted(required_text(node, "selector")?)
    ));
    if let Some(projection) = node.get("projection") {
        properties.push(format!(
            "projection:{}",
            render_projection(required_object(projection, "nodes[].projection")?)?
        ));
    }
    Ok(format!(
        "(lang_{}:Language {{id:{}}})-[:RESULTS]->[({id}:{label} {{{}}})]",
        gql_alias(language),
        quoted(language),
        properties.join(",")
    ))
}

fn render_projection(
    projection: &Map<String, Value>,
) -> Result<String, SearchTopologySettlementError> {
    let mut properties = vec![
        format!("rank:{}", required_u64(projection, "rank")?),
        format!("depth:{}", required_u64(projection, "depth")?),
    ];
    if let Some(hit) = projection.get("hit") {
        let hit = required_object(hit, "projection.hit")?;
        let mut hit_properties = Vec::new();
        if let Some(value) = hit.get("fd").and_then(Value::as_bool) {
            hit_properties.push(format!("fd:{value}"));
        }
        if let Some(value) = hit.get("rg") {
            hit_properties.push(format!(
                "rg:{}",
                serde_json::to_string(value)
                    .map_err(|failure| error("rendering-failed", failure.to_string()))?
            ));
        }
        if let Some(value) = hit.get("tantivy") {
            hit_properties.push(format!(
                "tantivy:{}",
                serde_json::to_string(value)
                    .map_err(|failure| error("rendering-failed", failure.to_string()))?
            ));
        }
        if let Some(value) = hit.get("native").and_then(Value::as_bool) {
            hit_properties.push(format!("native:{value}"));
        }
        properties.push(format!("hit:{{{}}}", hit_properties.join(",")));
    }
    if let Some(jq) = projection.get("jq").and_then(Value::as_str) {
        properties.push(format!("jq:{}", quoted(jq)));
    }
    Ok(format!("{{{}}}", properties.join(",")))
}

fn render_edge(edge: &Map<String, Value>) -> Result<String, SearchTopologySettlementError> {
    let modality = required_text(edge, "modality")?;
    let mut properties = vec![format!("modality:{}", quoted(modality))];
    if let Some(derived_by) = edge.get("derivedBy").and_then(Value::as_str) {
        properties.push(format!("derived_by:{}", quoted(derived_by)));
    }
    if let Some(proof) = edge.get("proofRef").and_then(Value::as_str) {
        properties.push(format!("proof:{}", quoted(proof)));
    }
    properties.push(format!(
        "witnesses:{}",
        render_text_array(edge, "witnesses")?
    ));
    Ok(format!(
        "({})-[:{} {{{}}}]->({})",
        required_text(edge, "from")?,
        required_text(edge, "relation")?,
        properties.join(","),
        required_text(edge, "to")?,
    ))
}

fn render_text_array(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, SearchTopologySettlementError> {
    serde_json::to_string(&text_array(object, field)?)
        .map_err(|failure| error("rendering-failed", failure.to_string()))
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn gql_alias(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn gql_type_prefix(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn is_blake3_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
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

fn error(reason_kind: &'static str, message: impl Into<String>) -> SearchTopologySettlementError {
    SearchTopologySettlementError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, SearchTopologySettlementError> {
    Err(error(reason_kind, message))
}

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, SearchTopologySettlementError> {
    value
        .as_object()
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

fn required_object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an object")))
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error("schema-invalid", format!("{field} must be an array")))
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, SearchTopologySettlementError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error("schema-invalid", format!("{field} must be non-empty text")))
}

fn required_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, SearchTopologySettlementError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        error(
            "schema-invalid",
            format!("{field} must be an unsigned integer"),
        )
    })
}

fn require_text_eq(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), SearchTopologySettlementError> {
    let observed = required_text(object, field)?;
    if observed != expected {
        return invalid(
            "schema-identity-drift",
            format!("{field} expected {expected} observed {observed}"),
        );
    }
    Ok(())
}

fn text_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Vec<&'a str>, SearchTopologySettlementError> {
    required_array(object, field)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    error(
                        "schema-invalid",
                        format!("{field} entries must be non-empty text"),
                    )
                })
        })
        .collect()
}
