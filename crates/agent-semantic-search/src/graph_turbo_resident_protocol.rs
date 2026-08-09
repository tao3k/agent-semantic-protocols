use serde::{Deserialize, Serialize};

pub const GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID: &str =
    "agent.semantic-protocols.graph-turbo-resident-server";
pub const GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphTurboMessageKind {
    Hello,
    LoadGeneration,
    ApplyDelta,
    Rank,
    Cancel,
    Health,
    Shutdown,
    Receipt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphTurboServerState {
    Building,
    Ready,
    Completed,
    Cancelled,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboResidentRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub message_kind: GraphTurboMessageKind,
    pub request_id: u64,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub protocol_digest: Option<String>,
    pub page_roots: std::collections::BTreeMap<String, String>,
    pub terms: Vec<String>,
    pub profile: Option<String>,
    pub budget: Option<u64>,
    pub deadline_unix_micros: Option<u64>,
    #[serde(default)]
    pub rank_payload: Option<serde_json::Value>,
    #[serde(default)]
    pub generation_payload: Option<serde_json::Value>,
}

impl GraphTurboResidentRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID {
            return Err("Graph Turbo request schemaId is not current".to_owned());
        }
        if self.schema_version != GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION {
            return Err("Graph Turbo request schemaVersion is not current".to_owned());
        }
        if self.request_id == 0 {
            return Err("Graph Turbo requestId must be positive".to_owned());
        }
        if self.workspace_identity.trim().is_empty() {
            return Err("Graph Turbo workspaceIdentity must be non-empty".to_owned());
        }
        validate_digest(&self.generation_digest, "generationDigest")?;
        if let Some(protocol_digest) = self.protocol_digest.as_deref() {
            validate_digest(protocol_digest, "protocolDigest")?;
        }
        for (section, digest) in &self.page_roots {
            if section.trim().is_empty() {
                return Err("Graph Turbo page root section must be non-empty".to_owned());
            }
            validate_digest(digest, "pageRoot")?;
        }
        if self.terms.iter().any(|term| term.trim().is_empty()) {
            return Err("Graph Turbo terms must be non-empty".to_owned());
        }
        if self
            .rank_payload
            .as_ref()
            .is_some_and(|payload| !payload.is_object())
        {
            return Err("Graph Turbo rankPayload must be an object".to_owned());
        }
        if self
            .generation_payload
            .as_ref()
            .is_some_and(|payload| !payload.is_object())
        {
            return Err("Graph Turbo generationPayload must be an object".to_owned());
        }
        match self.message_kind {
            GraphTurboMessageKind::LoadGeneration if self.generation_payload.is_none() => {
                return Err("Graph Turbo load-generation requires generationPayload".to_owned());
            }
            GraphTurboMessageKind::Rank if self.rank_payload.is_none() => {
                return Err("Graph Turbo rank requires rankPayload".to_owned());
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboRankedNode {
    pub node_id: String,
    pub score: f64,
    pub evidence_digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboResidentReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub message_kind: GraphTurboMessageKind,
    pub request_id: u64,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub state: GraphTurboServerState,
    pub reason_kind: Option<String>,
    pub ranked_nodes: Vec<GraphTurboRankedNode>,
    pub process_spawns: u64,
    pub generation_loads: u64,
    pub graph_page_reads: u64,
}

impl GraphTurboResidentReceipt {
    pub fn validate_for(&self, request: &GraphTurboResidentRequest) -> Result<(), String> {
        if self.schema_id != GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_ID
            || self.schema_version != GRAPH_TURBO_RESIDENT_SERVER_SCHEMA_VERSION
        {
            return Err("Graph Turbo receipt contract is not current".to_owned());
        }
        if self.message_kind != GraphTurboMessageKind::Receipt {
            return Err("Graph Turbo response is not a receipt".to_owned());
        }
        if self.request_id != request.request_id {
            return Err("Graph Turbo receipt requestId mismatch".to_owned());
        }
        if self.workspace_identity != request.workspace_identity {
            return Err("Graph Turbo receipt workspaceIdentity mismatch".to_owned());
        }
        if self.generation_digest != request.generation_digest {
            return Err("Graph Turbo receipt generationDigest mismatch".to_owned());
        }
        for node in &self.ranked_nodes {
            if node.node_id.trim().is_empty() || !node.score.is_finite() {
                return Err("Graph Turbo ranked node is invalid".to_owned());
            }
            validate_digest(&node.evidence_digest, "evidenceDigest")?;
        }
        Ok(())
    }
}

fn validate_digest(value: &str, field: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return Err(format!("Graph Turbo {field} must use blake3-256"));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "Graph Turbo {field} must contain 32 lowercase hex bytes"
        ));
    }
    if hex.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(format!("Graph Turbo {field} must use lowercase hex"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/graph_turbo_resident_protocol.rs"]
mod tests;
