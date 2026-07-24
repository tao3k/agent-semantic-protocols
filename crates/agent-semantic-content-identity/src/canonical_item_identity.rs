use serde::{Deserialize, Serialize};

pub const CANONICAL_ITEM_SELECTOR_SCHEMA_ID: &str = "asp.canonical-item-selector.v1";
pub const CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION: &str = "v1";

macro_rules! canonical_item_text_v1 {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

canonical_item_text_v1!(
    /// Stable identity for one complete active ASP artifact set.
    ActiveArtifactSetIdV1
);

canonical_item_text_v1!(
    /// Canonical relation between a scope frame and the selected item.
    CanonicalItemScopeRelationV1
);
canonical_item_text_v1!(
    /// Canonical language-neutral scope kind.
    CanonicalItemScopeKindV1
);
canonical_item_text_v1!(
    /// Canonical scope symbol.
    CanonicalItemScopeSymbolV1
);
canonical_item_text_v1!(
    /// Canonical language id for a selected item.
    CanonicalItemLanguageIdV1
);
canonical_item_text_v1!(
    /// Canonical language-neutral item kind.
    CanonicalItemKindV1
);
canonical_item_text_v1!(
    /// Canonical selected item symbol.
    CanonicalItemSymbolV1
);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalItemScopeV1 {
    pub relation: CanonicalItemScopeRelationV1,
    pub kind: CanonicalItemScopeKindV1,
    pub symbol: CanonicalItemScopeSymbolV1,
}

impl CanonicalItemScopeV1 {
    pub fn new(
        relation: impl Into<CanonicalItemScopeRelationV1>,
        kind: impl Into<CanonicalItemScopeKindV1>,
        symbol: impl Into<CanonicalItemScopeSymbolV1>,
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
    pub language_id: CanonicalItemLanguageIdV1,
    pub kind: CanonicalItemKindV1,
    pub symbol: CanonicalItemSymbolV1,
    pub scopes: Vec<CanonicalItemScopeV1>,
}

impl CanonicalItemIdentityV1 {
    pub fn new(
        language_id: impl Into<CanonicalItemLanguageIdV1>,
        kind: impl Into<CanonicalItemKindV1>,
        symbol: impl Into<CanonicalItemSymbolV1>,
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
        relation: impl Into<CanonicalItemScopeRelationV1>,
        kind: impl Into<CanonicalItemScopeKindV1>,
        symbol: impl Into<CanonicalItemScopeSymbolV1>,
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
    pub language_id: CanonicalItemLanguageIdV1,
    pub kind: CanonicalItemKindV1,
    pub symbol: CanonicalItemSymbolV1,
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
        let (language_id, selector_body) =
            self.structural_selector.split_once("://").ok_or_else(|| {
                "canonical item structuralSelector must include <language>://".to_string()
            })?;
        if language_id != self.language_id.as_str() {
            return Err(
                "canonical item structuralSelector language does not match languageId".to_string(),
            );
        }
        let (owner_path, identity_path) = selector_body.split_once('#').ok_or_else(|| {
            "canonical item structuralSelector must include an owner and item fragment".to_string()
        })?;
        if owner_path.trim().is_empty() {
            return Err(
                "canonical item structuralSelector owner path must not be empty".to_string(),
            );
        }
        let decoded = crate::structural_selector::decode_canonical_item_identity_path(
            &crate::structural_selector::StructuralSelectorLanguageId::from(
                self.language_id.as_str(),
            ),
            &crate::structural_selector::CanonicalItemIdentityPath::from(identity_path),
        )
        .map_err(|error| format!("canonical item structuralSelector is invalid: {error}"))?;
        if decoded.kind != self.kind
            || decoded.symbol != self.symbol
            || decoded.scopes != self.scopes
        {
            return Err(
                "canonical item structuralSelector identity does not match typed identity"
                    .to_string(),
            );
        }
        Ok(())
    }
}
