// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical structural-selector encoding shared by language providers.

mod component;
mod error;
mod identity_path;

pub use component::decode_structural_selector_component;
pub use component::decode_structural_selector_owner_path;
pub use component::encode_structural_selector_component;
pub use component::encode_structural_selector_owner_path;
pub use error::StructuralSelectorCodecError;
pub use identity_path::CanonicalItemIdentityPath;
pub use identity_path::StructuralSelectorLanguageId;
pub use identity_path::decode_canonical_item_identity_path;
pub use identity_path::encode_canonical_item_identity_path;
