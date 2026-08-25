//! Canonical structural-selector encoding shared by language providers.

mod component;
mod error;
mod identity_path;

pub use component::{
    decode_structural_selector_component, decode_structural_selector_owner_path,
    encode_structural_selector_component, encode_structural_selector_owner_path,
};
pub use error::StructuralSelectorCodecError;
pub use identity_path::{
    CanonicalItemIdentityPath, StructuralSelectorLanguageId, decode_canonical_item_identity_path,
    encode_canonical_item_identity_path,
};
