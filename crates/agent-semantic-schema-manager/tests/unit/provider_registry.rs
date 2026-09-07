// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use serde_json::Value;

use crate::build_support::provider_registry::resolve_provider_register;

#[test]
fn resolves_canonical_provider_registration_from_the_install_register() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = resolve_provider_register(source_root).unwrap();
    assert!(result.input_paths.len() > 2);
    assert!(
        result.identities.iter().any(|identity| {
            identity.language_id == "rust" && identity.provider_id == "asp-rust"
        })
    );
    let register: Value = serde_json::from_slice(&result.bytes).unwrap();
    let rust = register["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["languageId"] == "rust")
        .unwrap();
    assert!(rust.get("searchCapabilities").is_some());
    assert!(rust.get("queryPackDescriptor").is_some());
}
