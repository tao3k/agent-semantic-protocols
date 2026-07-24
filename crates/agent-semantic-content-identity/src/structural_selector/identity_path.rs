use crate::canonical_item_identity::{CanonicalItemIdentityV1, CanonicalItemScopeV1};

use super::{
    StructuralSelectorCodecError, decode_structural_selector_component,
    encode_structural_selector_component,
};

#[derive(Clone, Debug, Eq, PartialEq)]
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

pub fn encode_canonical_item_identity_path(identity: &CanonicalItemIdentityV1) -> String {
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

pub fn decode_canonical_item_identity_path(
    language_id: &StructuralSelectorLanguageId,
    encoded: &CanonicalItemIdentityPath,
) -> Result<CanonicalItemIdentityV1, StructuralSelectorCodecError> {
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
    let mut identity = CanonicalItemIdentityV1::new(
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
        identity.scopes.push(CanonicalItemScopeV1::new(
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
