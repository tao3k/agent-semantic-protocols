// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Semantic admission for an immutable, reusable Project Topology generation.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

use serde_json::Map;
use serde_json::Value;

use agent_semantic_content_identity::ProjectWorkspaceBinding;

pub const PROJECT_TOPOLOGY_LIBRARY_SCHEMA_ID: &str =
    "agent.semantic-protocols.project-topology-library";
pub const PROJECT_TOPOLOGY_LIBRARY_SCHEMA_VERSION: &str = "1";

/// The deterministic admitted topology edge rendered beside one Query result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyDisplayRelationship {
    edge_id: String,
    from_node: String,
    relation: String,
    to_node: String,
}

impl ProjectTopologyDisplayRelationship {
    pub fn edge_id(&self) -> &str {
        &self.edge_id
    }

    pub fn from_node(&self) -> &str {
        &self.from_node
    }

    pub fn relation(&self) -> &str {
        &self.relation
    }

    pub fn to_node(&self) -> &str {
        &self.to_node
    }
}

/// A complete Project Topology generation admitted for cross-consumer reuse.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectTopologyLibrary {
    packet: Value,
    project_workspace: ProjectWorkspaceBinding,
    node_count: usize,
    segment_count: usize,
    consumers: BTreeSet<String>,
}

impl ProjectTopologyLibrary {
    /// Validate cross-record invariants before Runtime publication.
    pub fn admit_with_receipts(
        packet: Value,
        manifest_project_workspace: &ProjectWorkspaceBinding,
        admitted_receipts: &BTreeMap<String, Value>,
    ) -> Result<Self, ProjectTopologyLibraryError> {
        let object = required_object(&packet, "packet")?;
        require_text_eq(object, "schemaId", PROJECT_TOPOLOGY_LIBRARY_SCHEMA_ID)?;
        require_text_eq(
            object,
            "schemaVersion",
            PROJECT_TOPOLOGY_LIBRARY_SCHEMA_VERSION,
        )?;
        let project_workspace = required_object_field(object, "projectWorkspace")?;
        let decoded_project_workspace: ProjectWorkspaceBinding =
            serde_json::from_value(Value::Object(project_workspace.clone())).map_err(|error| {
                crate::project_topology_library::error(
                    "topology-project-workspace-schema-mismatch",
                    format!("projectWorkspace cannot be decoded: {error}"),
                )
            })?;
        decoded_project_workspace.validate().map_err(|error| {
            crate::project_topology_library::error(error.reason_kind(), error.to_string())
        })?;
        if &decoded_project_workspace != manifest_project_workspace {
            return invalid(
                "topology-project-workspace-mismatch",
                "topology library does not equal the parser-owned manifest binding",
            );
        }
        for field in [
            "sourceGenerationDigest",
            "providerCatalogDigest",
            "libraryDigest",
        ] {
            require_digest(object, field)?;
        }
        let library_digest = required_text(object, "libraryDigest")?;
        let source_generation_digest = required_text(object, "sourceGenerationDigest")?;

        let identities = required_object_field(object, "identities")?;
        for field in [
            "structuralTopologyDigest",
            "semanticTopologyDigest",
            "inferenceProgramDigest",
            "providerGrammarDigest",
            "resolverDigest",
            "topologySchemaDigest",
        ] {
            require_digest(identities, field)?;
        }
        let semantic_digest = required_text(identities, "semanticTopologyDigest")?;
        let inference_digest = required_text(identities, "inferenceProgramDigest")?;

        let generation = required_object_field(object, "generation")?;
        require_text_eq(generation, "state", "complete")?;
        for field in [
            "generationDigest",
            "changeSetDigest",
            "fromScratchEquivalentDigest",
        ] {
            require_digest(generation, field)?;
        }
        if required_text(generation, "fromScratchEquivalentDigest")? != library_digest {
            return invalid(
                "topology-from-scratch-equivalence-mismatch",
                "incremental generation does not equal its from-scratch rebuild",
            );
        }
        let generation_digest = required_text(generation, "generationDigest")?;
        let rebuild_receipt = required_object_field(object, "fromScratchRebuildReceipt")?;
        let rebuild_receipt_id = required_text(rebuild_receipt, "id")?;
        if admitted_receipts.get(rebuild_receipt_id)
            != Some(&Value::Object(rebuild_receipt.clone()))
        {
            return invalid(
                "topology-rebuild-receipt-unadmitted",
                "embedded rebuild receipt is not independently admitted",
            );
        }
        if required_text(rebuild_receipt, "sourceGenerationDigest")? != source_generation_digest
            || required_text(rebuild_receipt, "inferenceProgramDigest")? != inference_digest
            || required_text(rebuild_receipt, "topologyGenerationDigest")? != generation_digest
            || required_text(rebuild_receipt, "recomputedLibraryDigest")? != library_digest
        {
            return invalid(
                "topology-rebuild-receipt-mismatch",
                "rebuild receipt does not bind the candidate generation and library",
            );
        }

        let mut semantic_receipts = BTreeMap::<String, &Map<String, Value>>::new();
        for receipt in required_array(object, "semanticAdmissionReceipts")? {
            let receipt = required_object(receipt, "semanticAdmissionReceipts[]")?;
            let id = required_text(receipt, "id")?.to_owned();
            if semantic_receipts.insert(id.clone(), receipt).is_some() {
                return invalid(
                    "topology-duplicate-semantic-admission-receipt",
                    format!("duplicate semantic admission receipt {id}"),
                );
            }
        }

        let mut segments = BTreeMap::<String, SegmentEvidence>::new();
        for segment in required_array(object, "segments")? {
            let segment = required_object(segment, "segments[]")?;
            let id = required_text(segment, "id")?.to_owned();
            let evidence = SegmentEvidence {
                skeleton_digest: required_text(segment, "skeletonDigest")?.to_owned(),
                node_ids: text_set(segment, "nodeIds")?,
                edge_ids: text_set(segment, "edgeIds")?,
            };
            if segments.insert(id.clone(), evidence).is_some() {
                return invalid(
                    "topology-duplicate-segment-id",
                    format!("duplicate segment {id}"),
                );
            }
        }
        for rebuilt in text_set(generation, "rebuiltSegmentIds")? {
            if !segments.contains_key(&rebuilt) {
                return invalid(
                    "topology-rebuilt-segment-unresolved",
                    format!("rebuilt segment {rebuilt} is absent"),
                );
            }
        }

        let mut nodes = BTreeMap::<String, Option<String>>::new();
        let mut node_kinds = BTreeMap::<String, String>::new();
        let mut source_nodes = BTreeMap::<String, String>::new();
        for node in required_array(object, "nodes")? {
            let node = required_object(node, "nodes[]")?;
            let id = required_text(node, "id")?.to_owned();
            let kind = required_text(node, "kind")?.to_owned();
            let plane = required_text(node, "plane")?;
            let segment_id = optional_text(node, "segmentId")?.map(str::to_owned);
            match (plane, segment_id.as_deref()) {
                ("synthesized-semantic", None) => {}
                ("synthesized-semantic", Some(_)) => {
                    return invalid(
                        "topology-synthesized-node-source-segment-forbidden",
                        format!("synthesized node {id} cannot claim source-segment ownership"),
                    );
                }
                (_, Some(segment_id)) if segments.contains_key(segment_id) => {
                    source_nodes.insert(id.clone(), segment_id.to_owned());
                }
                (_, Some(segment_id)) => {
                    return invalid(
                        "topology-node-segment-unresolved",
                        format!("node {id} references absent segment {segment_id}"),
                    );
                }
                (_, None) => {
                    return invalid(
                        "topology-source-node-segment-missing",
                        format!("source-backed node {id} has no segment"),
                    );
                }
            }
            if nodes.insert(id.clone(), segment_id).is_some() {
                return invalid("topology-duplicate-node-id", format!("duplicate node {id}"));
            }
            node_kinds.insert(id.clone(), kind);
            if let Some(annotation) = node.get("annotation") {
                let annotation = required_object(annotation, "nodes[].annotation")?;
                if required_text(annotation, "bindingDigest")? != semantic_digest {
                    return invalid(
                        "topology-annotation-binding-mismatch",
                        format!("annotation {id} is bound to another semantic topology"),
                    );
                }
                if annotation.get("state").and_then(Value::as_str) == Some("accepted") {
                    let receipt_id = required_text(annotation, "admissionReceiptRef")?;
                    let receipt = semantic_receipts.get(receipt_id).ok_or_else(|| {
                        error(
                            "topology-annotation-admission-receipt-mismatch",
                            format!("annotation {id} has no matching receipt"),
                        )
                    })?;
                    if admitted_receipts.get(receipt_id) != Some(&Value::Object((*receipt).clone()))
                        || required_text(receipt, "annotationNodeId")? != id
                        || required_text(receipt, "semanticTopologyDigest")? != semantic_digest
                    {
                        return invalid(
                            "topology-annotation-admission-receipt-mismatch",
                            format!("annotation {id} receipt binding is invalid"),
                        );
                    }
                }
            }
        }
        for removed in text_set(generation, "removedNodeIds")? {
            if nodes.contains_key(&removed) {
                return invalid(
                    "topology-removed-node-still-active",
                    format!("removed node {removed} remains active"),
                );
            }
        }
        validate_segment_membership(&segments, &source_nodes, |segment| &segment.node_ids)
            .map_err(|message| error("topology-segment-node-membership-mismatch", message))?;

        let mut edges = BTreeMap::<String, Option<String>>::new();
        let mut source_edges = BTreeMap::<String, String>::new();
        let mut derived_edges = BTreeSet::new();
        for edge in required_array(object, "edges")? {
            let edge = required_object(edge, "edges[]")?;
            let id = required_text(edge, "id")?.to_owned();
            let segment_id = optional_text(edge, "segmentId")?.map(str::to_owned);
            let from = required_text(edge, "from")?;
            let to = required_text(edge, "to")?;
            if !nodes.contains_key(from) || !nodes.contains_key(to) {
                return invalid(
                    "topology-dangling-edge",
                    format!("edge {id} has an absent endpoint"),
                );
            }
            if edges.insert(id.clone(), segment_id.clone()).is_some() {
                return invalid("topology-duplicate-edge-id", format!("duplicate edge {id}"));
            }
            let modality = required_text(edge, "modality")?;
            let binding = required_text(edge, "bindingDigest")?;
            let expected_binding = match modality {
                "parser-direct" | "declared" => {
                    let segment_id = segment_id.as_deref().ok_or_else(|| {
                        error(
                            "topology-source-edge-segment-missing",
                            format!("source-backed edge {id} has no segment"),
                        )
                    })?;
                    let segment = segments.get(segment_id).ok_or_else(|| {
                        error(
                            "topology-edge-segment-unresolved",
                            format!("edge {id} references absent segment {segment_id}"),
                        )
                    })?;
                    source_edges.insert(id.clone(), segment_id.to_owned());
                    segment.skeleton_digest.as_str()
                }
                "derived" => inference_digest,
                "proposed" => semantic_digest,
                other => {
                    return invalid(
                        "topology-edge-modality-unsupported",
                        format!("unsupported modality {other}"),
                    );
                }
            };
            if matches!(modality, "derived" | "proposed") && segment_id.is_some() {
                return invalid(
                    "topology-inferred-edge-source-segment-forbidden",
                    format!("inferred edge {id} cannot claim source-segment ownership"),
                );
            }
            if binding != expected_binding {
                return invalid(
                    "topology-edge-binding-mismatch",
                    format!("edge {id} is not bound to its modality authority"),
                );
            }
            if modality == "derived" {
                required_text(edge, "proofRef")?;
                derived_edges.insert(id);
            }
        }
        let removed_edges = text_set(generation, "removedEdgeIds")?;
        for removed in &removed_edges {
            if edges.contains_key(removed) {
                return invalid(
                    "topology-removed-edge-still-active",
                    format!("removed edge {removed} remains active"),
                );
            }
        }
        validate_segment_membership(&segments, &source_edges, |segment| &segment.edge_ids)
            .map_err(|message| error("topology-segment-edge-membership-mismatch", message))?;

        validate_relation_frontiers(object, &node_kinds)?;

        let closure = required_object_field(object, "closure")?;
        require_text_eq(closure, "state", "stable")?;
        require_digest(closure, "digest")?;
        require_digest(closure, "proofDagDigest")?;
        if text_set(closure, "derivedEdgeIds")? != derived_edges {
            return invalid(
                "topology-derived-closure-mismatch",
                "stable closure does not inventory exactly the active derived edges",
            );
        }
        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for dependency in required_array(closure, "proofDependencies")? {
            let dependency = required_object(dependency, "closure.proofDependencies[]")?;
            let derived_id = required_text(dependency, "derivedEdgeId")?.to_owned();
            let premises = text_set(dependency, "premiseEdgeIds")?;
            if dependencies.insert(derived_id.clone(), premises).is_some() {
                return invalid(
                    "topology-derived-dependency-duplicate",
                    format!("duplicate dependency record {derived_id}"),
                );
            }
        }
        if dependencies.keys().cloned().collect::<BTreeSet<_>>() != derived_edges {
            return invalid(
                "topology-derived-dependency-missing",
                "derived closure does not have one dependency record per derived edge",
            );
        }
        for premises in dependencies.values() {
            if !premises.is_disjoint(&removed_edges) {
                return invalid(
                    "topology-derived-dependency-invalidated",
                    "derived closure retains a dependency on a removed premise",
                );
            }
            if premises.iter().any(|premise| !edges.contains_key(premise)) {
                return invalid(
                    "topology-derived-dependency-unresolved",
                    "derived closure references an unavailable premise",
                );
            }
        }

        let consumers = text_set(object, "consumers")?;
        let terminal = required_object_field(object, "terminal")?;
        if required_u64(terminal, "terminalCount")? != 1 {
            return invalid(
                "topology-terminal-count-mismatch",
                "topology requires one terminal",
            );
        }

        Ok(Self {
            packet,
            project_workspace: decoded_project_workspace,
            node_count: nodes.len(),
            segment_count: segments.len(),
            consumers,
        })
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn segment_count(&self) -> usize {
        self.segment_count
    }

    pub fn supports_consumer(&self, consumer: &str) -> bool {
        self.consumers.contains(consumer)
    }

    pub fn as_json(&self) -> &Value {
        &self.packet
    }

    pub fn project_workspace(&self) -> &ProjectWorkspaceBinding {
        &self.project_workspace
    }

    pub fn project_workspace_identity(&self) -> &str {
        self.packet["projectWorkspace"]["projectWorkspaceIdentity"]
            .as_str()
            .expect("admitted project workspace identity")
    }

    pub fn workspace_root_path(&self) -> &str {
        self.packet["projectWorkspace"]["workspaceRootPath"]
            .as_str()
            .expect("admitted workspace root path")
    }

    pub fn source_generation_digest(&self) -> &str {
        self.packet["sourceGenerationDigest"]
            .as_str()
            .expect("admitted source generation digest")
    }

    pub fn provider_catalog_digest(&self) -> &str {
        self.packet["providerCatalogDigest"]
            .as_str()
            .expect("admitted provider catalog digest")
    }

    pub fn library_digest(&self) -> &str {
        self.packet["libraryDigest"]
            .as_str()
            .expect("admitted library digest")
    }

    pub fn generation_digest(&self) -> &str {
        self.packet["generation"]["generationDigest"]
            .as_str()
            .expect("admitted topology generation digest")
    }

    pub fn structural_topology_digest(&self) -> &str {
        self.packet["identities"]["structuralTopologyDigest"]
            .as_str()
            .expect("admitted structural topology digest")
    }

    pub fn semantic_topology_digest(&self) -> &str {
        self.packet["identities"]["semanticTopologyDigest"]
            .as_str()
            .expect("admitted semantic topology digest")
    }

    pub fn inference_program_digest(&self) -> &str {
        self.packet["identities"]["inferenceProgramDigest"]
            .as_str()
            .expect("admitted inference program digest")
    }

    pub fn closure_digest(&self) -> &str {
        self.packet["closure"]["digest"]
            .as_str()
            .expect("admitted topology closure digest")
    }

    /// Resolve the canonical incident topology edge for one exact selector.
    ///
    /// Edge identity, rather than packet order, owns the deterministic choice.
    /// This is a presentation projection only; it does not prescribe execution.
    pub fn canonical_relationship_for_selector(
        &self,
        selector: &str,
    ) -> Result<ProjectTopologyDisplayRelationship, ProjectTopologyLibraryError> {
        let object = required_object(&self.packet, "packet")?;
        let nodes = required_array(object, "nodes")?;
        let matching = nodes
            .iter()
            .filter_map(|node| required_object(node, "nodes[]").ok())
            .filter(|node| node.get("selector").and_then(Value::as_str) == Some(selector))
            .collect::<Vec<_>>();
        let [selected_node] = matching.as_slice() else {
            return invalid(
                if matching.is_empty() {
                    "topology-query-selector-node-missing"
                } else {
                    "topology-query-selector-node-ambiguous"
                },
                "Query selector must resolve to exactly one admitted topology node",
            );
        };
        let selected_id = required_text(selected_node, "id")?;
        let mut incident = required_array(object, "edges")?
            .iter()
            .filter_map(|edge| required_object(edge, "edges[]").ok())
            .filter(|edge| {
                edge.get("from").and_then(Value::as_str) == Some(selected_id)
                    || edge.get("to").and_then(Value::as_str) == Some(selected_id)
            })
            .collect::<Vec<_>>();
        incident.sort_by_key(|edge| edge.get("id").and_then(Value::as_str).unwrap_or_default());
        let edge = incident.first().ok_or_else(|| {
            error(
                "topology-query-selector-relationship-missing",
                "Query selector topology node has no admitted incident relationship",
            )
        })?;
        let from_id = required_text(edge, "from")?;
        let to_id = required_text(edge, "to")?;
        let node_label = |id: &str| -> Result<String, ProjectTopologyLibraryError> {
            let node = nodes
                .iter()
                .filter_map(|node| required_object(node, "nodes[]").ok())
                .find(|node| node.get("id").and_then(Value::as_str) == Some(id))
                .ok_or_else(|| {
                    error("topology-dangling-edge", "topology edge endpoint is absent")
                })?;
            Ok(node
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| {
                    !name.trim().is_empty() && !name.contains(['\r', '\n']) && !name.contains("-->")
                })
                .unwrap_or(id)
                .to_owned())
        };
        Ok(ProjectTopologyDisplayRelationship {
            edge_id: required_text(edge, "id")?.to_owned(),
            from_node: node_label(from_id)?,
            relation: required_text(edge, "relation")?.to_owned(),
            to_node: node_label(to_id)?,
        })
    }
}

fn validate_relation_frontiers(
    object: &Map<String, Value>,
    node_kinds: &BTreeMap<String, String>,
) -> Result<(), ProjectTopologyLibraryError> {
    type RelationKey = (String, String, String, String, u64);

    let positive_edges = required_array(object, "edges")?
        .iter()
        .map(|edge| required_object(edge, "edges[]"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|edge| {
            matches!(
                edge.get("modality").and_then(Value::as_str),
                Some("parser-direct" | "declared" | "derived")
            )
        })
        .map(|edge| {
            Ok((
                required_text(edge, "from")?.to_owned(),
                required_text(edge, "to")?.to_owned(),
                required_text(edge, "relation")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeSet<_>, ProjectTopologyLibraryError>>()?;

    let mut certificates = BTreeMap::<String, (&str, &str, &str)>::new();
    for certificate in required_array(object, "coverageCertificates")? {
        let certificate = required_object(certificate, "coverageCertificates[]")?;
        let id = required_text(certificate, "id")?.to_owned();
        let value = (
            required_text(certificate, "relation")?,
            required_text(certificate, "targetKind")?,
            required_text(certificate, "scope")?,
        );
        if certificates.insert(id.clone(), value).is_some() {
            return invalid(
                "topology-coverage-duplicate",
                format!("duplicate coverage certificate {id}"),
            );
        }
        require_digest(certificate, "digest")?;
    }

    let mut frontiers = BTreeMap::<RelationKey, &Map<String, Value>>::new();
    for frontier in required_array(object, "frontiers")? {
        let frontier = required_object(frontier, "frontiers[]")?;
        let key = (
            required_text(frontier, "anchor")?.to_owned(),
            required_text(frontier, "target")?.to_owned(),
            required_text(frontier, "relation")?.to_owned(),
            required_text(frontier, "targetKind")?.to_owned(),
            required_u64(frontier, "depth")?,
        );
        if frontiers.insert(key, frontier).is_some() {
            return invalid(
                "topology-frontier-duplicate",
                "duplicate frontier for one expected relation",
            );
        }
    }

    let mut expected_keys = BTreeSet::new();
    let mut expected_ids = BTreeSet::new();
    let mut referenced_certificates = BTreeSet::new();
    for expected in required_array(object, "expectedRelations")? {
        let expected = required_object(expected, "expectedRelations[]")?;
        let id = required_text(expected, "id")?;
        if !expected_ids.insert(id) {
            return invalid(
                "topology-expected-relation-duplicate",
                format!("duplicate expected relation {id}"),
            );
        }
        let anchor = required_text(expected, "anchor")?;
        let target = required_text(expected, "target")?;
        let relation = required_text(expected, "relation")?;
        let target_kind = required_text(expected, "targetKind")?;
        let key = (
            anchor.to_owned(),
            target.to_owned(),
            relation.to_owned(),
            target_kind.to_owned(),
            required_u64(expected, "depth")?,
        );
        if !expected_keys.insert(key.clone()) {
            return invalid(
                "topology-expected-relation-duplicate",
                "two expectations describe the same relation obligation",
            );
        }
        if !node_kinds.contains_key(anchor)
            || node_kinds.get(target).map(String::as_str) != Some(target_kind)
        {
            return invalid(
                "topology-expected-relation-endpoint-mismatch",
                format!("expected relation {id} has an absent or mistyped endpoint"),
            );
        }
        let has_positive =
            positive_edges.contains(&(anchor.to_owned(), target.to_owned(), relation.to_owned()));
        let frontier = frontiers.get(&key).copied();
        if has_positive {
            if frontier.is_some() {
                return invalid(
                    "topology-positive-frontier-conflict",
                    format!("expected relation {id} has both a positive witness and a frontier"),
                );
            }
            continue;
        }
        let frontier = frontier.ok_or_else(|| {
            error(
                "topology-frontier-missing",
                format!("unsettled expected relation {id} has no frontier"),
            )
        })?;
        let coverage = required_text(expected, "coverage")?;
        let state = required_text(frontier, "state")?;
        let reason = required_text(frontier, "reason")?;
        let reference = frontier.get("coverageRef").and_then(Value::as_str);
        match coverage {
            "none"
                if state == "unknown"
                    && reason == "binding-not-established"
                    && reference.is_none() => {}
            "partial" if state == "unknown" && reason == "coverage-open" => {
                let reference = validate_frontier_coverage(
                    reference,
                    relation,
                    target_kind,
                    "partial",
                    &certificates,
                )?;
                referenced_certificates.insert(reference.to_owned());
            }
            "complete"
                if state == "certified-missing" && reason == "complete-coverage-no-witness" =>
            {
                let reference = validate_frontier_coverage(
                    reference,
                    relation,
                    target_kind,
                    "complete",
                    &certificates,
                )?;
                referenced_certificates.insert(reference.to_owned());
            }
            _ => {
                return invalid(
                    "topology-frontier-classification-mismatch",
                    format!("frontier for {id} is inconsistent with its coverage"),
                );
            }
        }
    }
    let unresolved_expected = expected_keys
        .iter()
        .filter(|key| !positive_edges.contains(&(key.0.clone(), key.1.clone(), key.2.clone())))
        .cloned()
        .collect::<BTreeSet<_>>();
    if frontiers.keys().cloned().collect::<BTreeSet<_>>() != unresolved_expected {
        return invalid(
            "topology-frontier-expectation-mismatch",
            "frontiers must equal exactly the unresolved expected relations",
        );
    }
    if referenced_certificates != certificates.keys().cloned().collect() {
        return invalid(
            "topology-frontier-coverage-extraneous",
            "coverage certificates must equal exactly the frontier references",
        );
    }
    Ok(())
}

fn validate_frontier_coverage<'a>(
    reference: Option<&'a str>,
    relation: &str,
    target_kind: &str,
    scope: &str,
    certificates: &BTreeMap<String, (&str, &str, &str)>,
) -> Result<&'a str, ProjectTopologyLibraryError> {
    let reference = reference.ok_or_else(|| {
        error(
            "topology-frontier-coverage-missing",
            "covered frontier has no coverage certificate reference",
        )
    })?;
    if certificates.get(reference).copied() != Some((relation, target_kind, scope)) {
        return invalid(
            "topology-frontier-coverage-mismatch",
            format!("coverage certificate {reference} does not match the frontier"),
        );
    }
    Ok(reference)
}

struct SegmentEvidence {
    skeleton_digest: String,
    node_ids: BTreeSet<String>,
    edge_ids: BTreeSet<String>,
}

fn validate_segment_membership(
    segments: &BTreeMap<String, SegmentEvidence>,
    actual: &BTreeMap<String, String>,
    declared: impl Fn(&SegmentEvidence) -> &BTreeSet<String>,
) -> Result<(), String> {
    let mut indexed = BTreeMap::<String, String>::new();
    for (segment_id, segment) in segments {
        for item_id in declared(segment) {
            if indexed
                .insert(item_id.clone(), segment_id.clone())
                .is_some()
            {
                return Err(format!("item {item_id} is indexed by multiple segments"));
            }
        }
    }
    if &indexed != actual {
        return Err("segment inventory does not equal active records".to_owned());
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyLibraryError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologyLibraryError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectTopologyLibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologyLibraryError {}

fn error(reason_kind: &'static str, message: impl Into<String>) -> ProjectTopologyLibraryError {
    ProjectTopologyLibraryError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, ProjectTopologyLibraryError> {
    Err(error(reason_kind, message))
}

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a Map<String, Value>, ProjectTopologyLibraryError> {
    value.as_object().ok_or_else(|| {
        error(
            "topology-schema-invalid",
            format!("{field} must be an object"),
        )
    })
}

fn required_object_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Map<String, Value>, ProjectTopologyLibraryError> {
    object.get(field).and_then(Value::as_object).ok_or_else(|| {
        error(
            "topology-schema-invalid",
            format!("{field} must be an object"),
        )
    })
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], ProjectTopologyLibraryError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            error(
                "topology-schema-invalid",
                format!("{field} must be an array"),
            )
        })
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, ProjectTopologyLibraryError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            error(
                "topology-schema-invalid",
                format!("{field} must be non-empty text"),
            )
        })
}

fn optional_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, ProjectTopologyLibraryError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value)),
        _ => Err(error(
            "topology-schema-invalid",
            format!("{field} must be non-empty text or null"),
        )),
    }
}

fn required_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<u64, ProjectTopologyLibraryError> {
    object.get(field).and_then(Value::as_u64).ok_or_else(|| {
        error(
            "topology-schema-invalid",
            format!("{field} must be an unsigned integer"),
        )
    })
}

fn require_text_eq(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), ProjectTopologyLibraryError> {
    let observed = required_text(object, field)?;
    if observed != expected {
        return invalid(
            "topology-schema-identity-drift",
            format!("{field} expected {expected} observed {observed}"),
        );
    }
    Ok(())
}

fn require_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), ProjectTopologyLibraryError> {
    let value = required_text(object, field)?;
    if !is_blake3_digest(value) {
        return invalid(
            "topology-schema-invalid",
            format!("{field} must be a blake3-256 digest"),
        );
    }
    Ok(())
}

fn text_set(
    object: &Map<String, Value>,
    field: &str,
) -> Result<BTreeSet<String>, ProjectTopologyLibraryError> {
    let values = required_array(object, field)?;
    let mut result = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                error(
                    "topology-schema-invalid",
                    format!("{field} entries must be non-empty text"),
                )
            })?;
        if !result.insert(value.to_owned()) {
            return invalid(
                "topology-schema-invalid",
                format!("{field} contains duplicate {value}"),
            );
        }
    }
    Ok(result)
}

fn is_blake3_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}
