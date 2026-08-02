use serde::{Deserialize, Serialize};

pub const PROVIDER_RELATION_GENERATION_SCHEMA_ID: &str =
    "asp.provider-relation-generation.v1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedRelationEndpoint {
    pub kind: String,
    pub id: String,
}

impl ProviderProjectedRelationEndpoint {
    pub fn validate(&self) -> Result<(), String> {
        if self.kind.trim().is_empty() || self.id.trim().is_empty() {
            return Err("provider projected relation endpoint is incomplete".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedRelation {
    pub from: ProviderProjectedRelationEndpoint,
    pub kind: String,
    pub to: ProviderProjectedRelationEndpoint,
}

impl ProviderProjectedRelation {
    pub fn validate(&self) -> Result<(), String> {
        self.from.validate()?;
        self.to.validate()?;
        if self.kind.trim().is_empty() {
            return Err("provider projected relation kind is empty".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRelationGeneration {
    pub schema_id: String,
    pub schema_version: String,
    pub generation_digest: String,
    pub relations: Vec<ProviderProjectedRelation>,
}

impl ProviderRelationGeneration {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROVIDER_RELATION_GENERATION_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("provider relation generation schema identity mismatch".to_owned());
        }
        let digest = self
            .generation_digest
            .strip_prefix("blake3-256:")
            .ok_or_else(|| "provider relation generation digest is not BLAKE3".to_owned())?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("provider relation generation digest is invalid".to_owned());
        }
        let mut unique = std::collections::BTreeSet::new();
        for relation in &self.relations {
            relation.validate()?;
            if !unique.insert(relation) {
                return Err("provider relation generation contains a duplicate edge".to_owned());
            }
        }
        Ok(())
    }
}
