//! Canonical typed identities for parser-owned items and structural selectors.

use serde::Deserialize;
use serde::Serialize;

/// Schema identifier for canonical parser-owned item selectors.
pub const CANONICAL_ITEM_SELECTOR_SCHEMA_ID: &str = "asp.canonical-item-selector.v1";
/// Schema version for canonical parser-owned item selectors.
pub const CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION: &str = "1";

macro_rules! canonical_item_text {
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

canonical_item_text!(
    /// Canonical relation between a scope frame and the selected item.
    CanonicalItemScopeRelation
);
canonical_item_text!(
    /// Canonical language-neutral scope kind.
    CanonicalItemScopeKind
);
canonical_item_text!(
    /// Canonical scope symbol.
    CanonicalItemScopeSymbol
);
canonical_item_text!(
    /// Canonical language id for a selected item.
    CanonicalItemLanguageId
);
canonical_item_text!(
    /// Canonical language-neutral item kind.
    CanonicalItemKind
);
canonical_item_text!(
    /// Canonical selected item symbol.
    CanonicalItemSymbol
);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// One typed scope segment in a canonical parser item identity.
pub struct CanonicalItemScope {
    pub relation: CanonicalItemScopeRelation,
    pub kind: CanonicalItemScopeKind,
    pub symbol: CanonicalItemScopeSymbol,
}

impl CanonicalItemScope {
    pub fn new(
        relation: impl Into<CanonicalItemScopeRelation>,
        kind: impl Into<CanonicalItemScopeKind>,
        symbol: impl Into<CanonicalItemScopeSymbol>,
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
/// Language-neutral identity of one parser-owned source item.
pub struct CanonicalItemIdentity {
    pub language_id: CanonicalItemLanguageId,
    pub kind: CanonicalItemKind,
    pub symbol: CanonicalItemSymbol,
    pub scopes: Vec<CanonicalItemScope>,
}

impl CanonicalItemIdentity {
    pub fn new(
        language_id: impl Into<CanonicalItemLanguageId>,
        kind: impl Into<CanonicalItemKind>,
        symbol: impl Into<CanonicalItemSymbol>,
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
        relation: impl Into<CanonicalItemScopeRelation>,
        kind: impl Into<CanonicalItemScopeKind>,
        symbol: impl Into<CanonicalItemScopeSymbol>,
    ) -> Self {
        self.scopes
            .push(CanonicalItemScope::new(relation, kind, symbol));
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
/// Structural selector paired with its decoded canonical item identity.
pub struct CanonicalItemSelector {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: CanonicalItemLanguageId,
    pub kind: CanonicalItemKind,
    pub symbol: CanonicalItemSymbol,
    pub scopes: Vec<CanonicalItemScope>,
    pub structural_selector: String,
}

impl CanonicalItemSelector {
    pub fn identity(&self) -> CanonicalItemIdentity {
        let mut identity = CanonicalItemIdentity::new(
            self.language_id.clone(),
            self.kind.clone(),
            self.symbol.clone(),
        );
        for scope in &self.scopes {
            identity = identity.with_scope(
                scope.relation.clone(),
                scope.kind.clone(),
                scope.symbol.clone(),
            );
        }
        identity
    }

    pub fn parse(structural_selector: impl Into<String>) -> Result<Self, String> {
        let structural_selector = structural_selector.into();
        let (language_id, selector_body) =
            structural_selector.split_once("://").ok_or_else(|| {
                "canonical item structuralSelector must include <language>://".to_string()
            })?;
        let (owner_path, identity_path) = selector_body.rsplit_once('#').ok_or_else(|| {
            "canonical item structuralSelector must include an owner and item fragment".to_string()
        })?;
        if owner_path.trim().is_empty() {
            return Err(
                "canonical item structuralSelector owner path must not be empty".to_string(),
            );
        }
        let identity = crate::structural_selector::decode_canonical_item_identity_path(
            &crate::structural_selector::StructuralSelectorLanguageId::from(language_id),
            &crate::structural_selector::CanonicalItemIdentityPath::from(identity_path),
        )
        .map_err(|error| format!("canonical item structuralSelector is invalid: {error}"))?;
        let selector = CanonicalItemSelector::new(identity, structural_selector);
        selector.validate()?;
        Ok(selector)
    }

    pub fn parse_root_or_exact_descendant(
        structural_selector: impl Into<String>,
    ) -> Result<Self, String> {
        let structural_selector = structural_selector.into();
        let mut parts = structural_selector.split("/segment/");
        let root = parts.next().unwrap_or_default();
        let descendants = parts.collect::<Vec<_>>();
        if descendants.is_empty() {
            return Self::parse(structural_selector);
        }
        for descendant in descendants {
            let (kind, identity) = descendant.split_once('/').ok_or_else(|| {
                "exact descendant structuralSelector segment must include <kind>/<identity>"
                    .to_string()
            })?;
            if kind.is_empty() || identity.is_empty() || identity.contains('/') {
                return Err(
                    "exact descendant structuralSelector segment must be a canonical <kind>/<identity> pair"
                        .to_string(),
                );
            }
        }
        Self::parse(root.to_string())
    }

    pub fn new(identity: CanonicalItemIdentity, structural_selector: impl Into<String>) -> Self {
        let CanonicalItemIdentity {
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

    /// Returns the parser-validated source owner carried by this selector.
    pub fn owner_path(&self) -> Result<String, String> {
        let encoded = self
            .structural_selector
            .split_once("://")
            .and_then(|(_, selector_body)| selector_body.rsplit_once('#'))
            .map(|(owner_path, _)| owner_path)
            .ok_or_else(|| {
                "canonical item structuralSelector must include an owner and item fragment"
                    .to_owned()
            })?;
        let decoded = crate::structural_selector::decode_structural_selector_owner_path(encoded)
            .map_err(|error| {
                format!("canonical item structuralSelector owner path is invalid: {error}")
            })?;
        if crate::structural_selector::encode_structural_selector_owner_path(&decoded) != encoded {
            return Err(
                "canonical item structuralSelector owner path is not canonically encoded"
                    .to_owned(),
            );
        }
        Ok(decoded)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_schema_identity()?;
        self.validate_required_identity_fields()?;
        self.validate_scope_fields()?;
        self.validate_structural_identity()
    }

    fn validate_schema_identity(&self) -> Result<(), String> {
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
        Ok(())
    }

    fn validate_required_identity_fields(&self) -> Result<(), String> {
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
        Ok(())
    }

    fn validate_scope_fields(&self) -> Result<(), String> {
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

    fn validate_structural_identity(&self) -> Result<(), String> {
        let (language_id, selector_body) =
            self.structural_selector.split_once("://").ok_or_else(|| {
                "canonical item structuralSelector must include <language>://".to_string()
            })?;
        if language_id != self.language_id.as_str() {
            return Err(
                "canonical item structuralSelector language does not match languageId".to_string(),
            );
        }
        let (owner_path, identity_path) = selector_body.rsplit_once('#').ok_or_else(|| {
            "canonical item structuralSelector must include an owner and item fragment".to_string()
        })?;
        if owner_path.trim().is_empty() {
            return Err(
                "canonical item structuralSelector owner path must not be empty".to_string(),
            );
        }
        self.owner_path()?;
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
