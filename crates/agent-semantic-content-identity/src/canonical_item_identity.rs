use serde::{Deserialize, Serialize};

pub const CANONICAL_ITEM_SELECTOR_SCHEMA_ID: &str = "asp.canonical-item-selector.v1";
pub const CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION: &str = "v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalItemScopeV1 {
    pub relation: String,
    pub kind: String,
    pub symbol: String,
}

impl CanonicalItemScopeV1 {
    pub fn new(
        relation: impl Into<String>,
        kind: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            relation: relation.into(),
            kind: kind.into(),
            symbol: symbol.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalItemIdentityV1 {
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub scopes: Vec<CanonicalItemScopeV1>,
}

impl CanonicalItemIdentityV1 {
    pub fn new(
        language_id: impl Into<String>,
        kind: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            kind: kind.into(),
            symbol: symbol.into(),
            scopes: Vec::new(),
        }
    }

    pub fn with_scope(
        mut self,
        relation: impl Into<String>,
        kind: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        self.scopes
            .push(CanonicalItemScopeV1::new(relation, kind, symbol));
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("languageId", self.language_id.as_str()),
            ("kind", self.kind.as_str()),
            ("symbol", self.symbol.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("canonical item identity {field} must not be empty"));
            }
        }
        for scope in &self.scopes {
            for (field, value) in [
                ("relation", scope.relation.as_str()),
                ("kind", scope.kind.as_str()),
                ("symbol", scope.symbol.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(format!(
                        "canonical item identity scope {field} must not be empty"
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalItemSelectorV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub scopes: Vec<CanonicalItemScopeV1>,
    pub structural_selector: String,
}

impl CanonicalItemSelectorV1 {
    pub fn new(identity: CanonicalItemIdentityV1, structural_selector: impl Into<String>) -> Self {
        let CanonicalItemIdentityV1 {
            language_id,
            kind,
            symbol,
            scopes,
        } = identity;
        Self {
            schema_id: CANONICAL_ITEM_SELECTOR_SCHEMA_ID.to_string(),
            schema_version: CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION.to_string(),
            language_id,
            kind,
            symbol,
            scopes,
            structural_selector: structural_selector.into(),
        }
    }

    pub fn structural_selector(&self) -> &str {
        &self.structural_selector
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CANONICAL_ITEM_SELECTOR_SCHEMA_ID {
            return Err(format!(
                "canonical item selector schemaId must be {CANONICAL_ITEM_SELECTOR_SCHEMA_ID}"
            ));
        }
        if self.schema_version != CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION {
            return Err(format!(
                "canonical item selector schemaVersion must be {CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION}"
            ));
        }
        for (field, value) in [
            ("languageId", self.language_id.as_str()),
            ("kind", self.kind.as_str()),
            ("symbol", self.symbol.as_str()),
            ("structuralSelector", self.structural_selector.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("canonical item selector {field} must not be empty"));
            }
        }
        for scope in &self.scopes {
            for (field, value) in [
                ("relation", scope.relation.as_str()),
                ("kind", scope.kind.as_str()),
                ("symbol", scope.symbol.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(format!(
                        "canonical item selector scope {field} must not be empty"
                    ));
                }
            }
        }
        Ok(())
    }
}
