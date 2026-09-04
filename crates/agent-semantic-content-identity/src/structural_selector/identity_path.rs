//! Maps canonical item identities to reversible structural-selector paths.

use crate::canonical_item_identity::CanonicalItemIdentity;
use crate::canonical_item_identity::CanonicalItemScope;

use super::StructuralSelectorCodecError;
use super::decode_structural_selector_component;
use super::encode_structural_selector_component;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Validated language identifier embedded in a structural-selector path.
pub struct StructuralSelectorLanguageId(String);

impl StructuralSelectorLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for StructuralSelectorLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for StructuralSelectorLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Canonical reversible path encoding of one parser item identity.
pub struct CanonicalItemIdentityPath(String);

impl CanonicalItemIdentityPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for CanonicalItemIdentityPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CanonicalItemIdentityPath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Encodes one canonical item identity as a selector path.
pub fn encode_canonical_item_identity_path(identity: &CanonicalItemIdentity) -> String {
    let mut encoded = format!(
        "item/{}/{}",
        encode_structural_selector_component(identity.kind.as_str()),
        encode_structural_selector_component(identity.symbol.as_str())
    );
    for scope in &identity.scopes {
        encoded.push_str("/scope/");
        encoded.push_str(&encode_structural_selector_component(
            scope.relation.as_str(),
        ));
        encoded.push('/');
        encoded.push_str(&encode_structural_selector_component(scope.kind.as_str()));
        encoded.push('/');
        encoded.push_str(&encode_structural_selector_component(scope.symbol.as_str()));
    }
    encoded
}

/// Decodes a selector path into its canonical item identity.
pub fn decode_canonical_item_identity_path(
    language_id: &StructuralSelectorLanguageId,
    encoded: &CanonicalItemIdentityPath,
) -> Result<CanonicalItemIdentity, StructuralSelectorCodecError> {
    let segments = encoded.as_str().split('/').collect::<Vec<_>>();
    if segments.len() < 3 || segments[0] != "item" {
        return Err(StructuralSelectorCodecError::new(
            "canonical item identity path must start with item/<kind>/<symbol>",
        ));
    }
    let trailing = &segments[3..];
    if trailing.len() % 4 != 0 {
        return Err(StructuralSelectorCodecError::new(
            "canonical item identity scope segments are incomplete",
        ));
    }
    let mut identity = CanonicalItemIdentity::new(
        language_id.as_str(),
        decode_structural_selector_component(segments[1])?,
        decode_structural_selector_component(segments[2])?,
    );
    for scope in trailing.chunks_exact(4) {
        if scope[0] != "scope" {
            return Err(StructuralSelectorCodecError::new(
                "canonical item identity trailing segment must start with scope",
            ));
        }
        identity.scopes.push(CanonicalItemScope::new(
            decode_structural_selector_component(scope[1])?,
            decode_structural_selector_component(scope[2])?,
            decode_structural_selector_component(scope[3])?,
        ));
    }
    identity
        .validate()
        .map_err(StructuralSelectorCodecError::new)?;
    let canonical = encode_canonical_item_identity_path(&identity);
    if canonical != encoded.as_str() {
        return Err(StructuralSelectorCodecError::new(
            "canonical item identity path is not in canonical encoding",
        ));
    }
    Ok(identity)
}
