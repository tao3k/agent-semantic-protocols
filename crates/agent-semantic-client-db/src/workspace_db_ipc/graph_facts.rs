//! Typed, generation-bound relation facts returned by the resident search data plane.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeGraphFactSource {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeGraphFactsRead {
    pub schema_id: String,
    pub schema_version: String,
    pub generation_digest: String,
    pub relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
}

impl RuntimeGraphFactsRead {
    pub const SCHEMA_ID: &'static str = "agent.semantic-protocols.runtime-search-generation-facts";

    pub fn new(
        generation_digest: String,
        relations: Vec<
            agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
        >,
    ) -> Result<Self, String> {
        let read = Self {
            schema_id: Self::SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            generation_digest,
            relations,
        };
        read.validate()?;
        Ok(read)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != Self::SCHEMA_ID || self.schema_version != "1" {
            return Err("runtime graph facts schema mismatch".to_owned());
        }
        let digest = self
            .generation_digest
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "runtime graph facts generation digest is unqualified".to_owned())?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("runtime graph facts generation digest is malformed".to_owned());
        }
        Ok(())
    }
}
