use std::collections::BTreeSet;

use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest;
use serde::Deserialize;
use serde::Serialize;

pub const SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-architecture-inventory";
pub const SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchArchitectureEdgeKind {
    CargoFeature,
    CargoDependency,
    RustReexport,
    RuntimeRoute,
    RustCall,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchArchitectureFactNode {
    pub node_id: String,
    pub source_selector: String,
    pub declared_owner: String,
    pub capability_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchArchitectureFactEdge {
    pub source_node_id: String,
    pub target_node_id: String,
    pub kind: SearchArchitectureEdgeKind,
    pub enabled: bool,
    pub evidence_selector: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchArchitectureFactInventory {
    pub schema_id: String,
    pub schema_version: String,
    pub inventory_digest: String,
    pub production_feature_set: Vec<String>,
    pub production_entries: Vec<String>,
    pub nodes: Vec<SearchArchitectureFactNode>,
    pub edges: Vec<SearchArchitectureFactEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchArchitectureInventoryError {
    EmptyProductionEntries,
    EmptyField {
        field: &'static str,
    },
    InvalidIdentifier {
        field: &'static str,
        value: String,
    },
    DuplicateNode(String),
    UnknownNodeReference {
        field: &'static str,
        node_id: String,
    },
    Serialization(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchArchitectureDigestPayload<'a> {
    schema_id: &'a str,
    schema_version: &'a str,
    production_feature_set: &'a [String],
    production_entries: &'a [String],
    nodes: &'a [SearchArchitectureFactNode],
    edges: &'a [SearchArchitectureFactEdge],
}

impl SearchArchitectureFactInventory {
    pub fn from_facts(
        mut production_feature_set: Vec<String>,
        mut production_entries: Vec<String>,
        mut nodes: Vec<SearchArchitectureFactNode>,
        mut edges: Vec<SearchArchitectureFactEdge>,
    ) -> Result<Self, SearchArchitectureInventoryError> {
        production_feature_set.sort();
        production_feature_set.dedup();
        production_entries.sort();
        production_entries.dedup();
        nodes.sort();
        edges.sort();
        edges.dedup();

        if production_entries.is_empty() {
            return Err(SearchArchitectureInventoryError::EmptyProductionEntries);
        }
        for value in production_feature_set
            .iter()
            .chain(production_entries.iter())
        {
            require_identifier("architecture identifier", value)?;
        }

        let mut node_ids = BTreeSet::new();
        for node in &mut nodes {
            require_identifier("nodeId", &node.node_id)?;
            require_non_empty("sourceSelector", &node.source_selector)?;
            require_identifier("declaredOwner", &node.declared_owner)?;
            node.capability_claims.sort();
            node.capability_claims.dedup();
            for capability in &node.capability_claims {
                require_identifier("capabilityClaims", capability)?;
            }
            if !node_ids.insert(node.node_id.clone()) {
                return Err(SearchArchitectureInventoryError::DuplicateNode(
                    node.node_id.clone(),
                ));
            }
        }

        for entry in &production_entries {
            require_node_reference("productionEntries", entry, &node_ids)?;
        }
        for edge in &edges {
            require_non_empty("evidenceSelector", &edge.evidence_selector)?;
            require_node_reference("sourceNodeId", &edge.source_node_id, &node_ids)?;
            require_node_reference("targetNodeId", &edge.target_node_id, &node_ids)?;
        }

        let payload = SearchArchitectureDigestPayload {
            schema_id: SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_ID,
            schema_version: SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_VERSION,
            production_feature_set: &production_feature_set,
            production_entries: &production_entries,
            nodes: &nodes,
            edges: &edges,
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|error| SearchArchitectureInventoryError::Serialization(error.to_string()))?;
        let inventory_digest = canonical_content_digest(
            b"agent.semantic-protocols.search-architecture-inventory.v1",
            &[&bytes],
        );
        let inventory_digest = format!("blake3-256:{}", inventory_digest.as_str());

        Ok(Self {
            schema_id: SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_ID.to_owned(),
            schema_version: SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_VERSION.to_owned(),
            inventory_digest,
            production_feature_set,
            production_entries,
            nodes,
            edges,
        })
    }
}

fn require_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), SearchArchitectureInventoryError> {
    if value.is_empty() {
        return Err(SearchArchitectureInventoryError::EmptyField { field });
    }
    Ok(())
}

fn require_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), SearchArchitectureInventoryError> {
    require_non_empty(field, value)?;
    let mut bytes = value.bytes();
    let first = bytes.next().expect("non-empty identifier");
    let first_valid = first.is_ascii_alphanumeric();
    let rest_valid = bytes.all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
    });
    if !first_valid || !rest_valid {
        return Err(SearchArchitectureInventoryError::InvalidIdentifier {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn require_node_reference(
    field: &'static str,
    node_id: &str,
    node_ids: &BTreeSet<String>,
) -> Result<(), SearchArchitectureInventoryError> {
    if !node_ids.contains(node_id) {
        return Err(SearchArchitectureInventoryError::UnknownNodeReference {
            field,
            node_id: node_id.to_owned(),
        });
    }
    Ok(())
}
