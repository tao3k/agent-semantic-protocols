//! Canonical structural-selector encoding shared by language providers.

mod component;
mod error;
mod identity_path;

pub use component::{decode_structural_selector_component, encode_structural_selector_component};
pub use error::StructuralSelectorCodecError;
pub use identity_path::{
    CanonicalItemIdentityPath, StructuralSelectorLanguageId, decode_canonical_item_identity_path,
    encode_canonical_item_identity_path,
};
