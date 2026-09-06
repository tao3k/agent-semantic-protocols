//! Runtime admission for a Search projection of the reusable Project Topology.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;

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
            ("workspaceIdentity", library.workspace_identity()),
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

        let binding = required_object_field(object, "binding")?;
        for field in [
            "workspaceIdentity",
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
            if field != "workspaceIdentity" && !is_blake3_digest(value) {
                return invalid(
                    "schema-invalid",
                    format!("binding.{field} must be a blake3-256 digest"),
                );
            }
        }
        let semantic_topology_digest = required_text(binding, "semanticTopologyDigest")?;
        let fixed_point = required_object_field(object, "fixedPoint")?;
        require_text_eq(fixed_point, "programId", "project-topology-inference.v1")?;
        require_text_eq(fixed_point, "state", "reached")?;
        require_text_eq(fixed_point, "terminationKind", "fixed-point")?;
        let candidate_relation_set_digest =
            required_text(fixed_point, "candidateRelationSetDigest")?;
        let next_relation_set_digest = required_text(fixed_point, "nextRelationSetDigest")?;
        if !is_blake3_digest(candidate_relation_set_digest)
            || !is_blake3_digest(next_relation_set_digest)
        {
            return invalid(
                "schema-invalid",
                "fixed-point relation-set identities must be blake3-256 digests",
            );
        }
        if candidate_relation_set_digest != next_relation_set_digest {
            return invalid(
                "fixed-point-not-reached",
                "candidate and next relation sets differ",
            );
        }
        if fixed_point
            .get("postRankingCertified")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return invalid(
                "post-ranking-uncertified",
                "post-ranking certificate is absent",
            );
        }

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
        if required_u64(fixed_point, "derivedRelationCount")? != derived_relation_count {
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
        if text_array(materialization, "proofDependencies")?
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
        let binding = required_object_field(packet, "binding")?;
        let fixed_point = required_object_field(packet, "fixedPoint")?;
        let mut lines = vec![format!(
            "#+SEARCH_EVIDENCE: v1 topology={} semantic={} logic={} fixed-point={}-{}",
            required_text(binding, "structuralTopologyDigest")?,
            required_text(binding, "semanticTopologyDigest")?,
            required_text(fixed_point, "programId")?,
            required_text(fixed_point, "scope")?,
            required_text(fixed_point, "state")?,
        )];
        lines.push(format!(
            "#+begin_src gql :profile search-evidence.v1 :binding {} :eval never",
            quoted(required_text(binding, "evidenceBindingDigest")?)
        ));

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
        lines.push("#+end_src".to_owned());
        Ok(format!("{}\n", lines.join("\n")))
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
