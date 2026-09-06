// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use crate::ClientDbSourceIndexImport;

pub(super) fn turso_source_index_selector_fingerprint(
    import: &ClientDbSourceIndexImport,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    fn update_text(hasher: &mut Sha256, value: &str) {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    hasher.update(b"asp.source-index-selector-fingerprint.v1\0");
    hasher.update((import.selectors.len() as u64).to_be_bytes());
    for selector in &import.selectors {
        update_text(&mut hasher, selector.owner_path.as_str());
        update_text(&mut hasher, selector.selector_id.as_str());
        update_text(
            &mut hasher,
            selector.symbol.as_ref().map_or("", |value| value.as_str()),
        );
        update_text(
            &mut hasher,
            selector.kind.as_ref().map_or("", |value| value.as_str()),
        );
        hasher.update((selector.query_keys.len() as u64).to_be_bytes());
        for query_key in &selector.query_keys {
            update_text(&mut hasher, query_key.as_str());
        }
        update_text(&mut hasher, selector.provider_id.as_str());
        let identity =
            serde_json::to_vec(selector.projection_record.proof.canonical_item_selector())
                .map_err(|error| {
                    format!("encode source-index selector canonical identity: {error}")
                })?;
        hasher.update((identity.len() as u64).to_be_bytes());
        hasher.update(identity);
        let projection = serde_json::to_vec(&selector.projection_record)
            .map_err(|error| format!("encode source-index selector projection record: {error}"))?;
        hasher.update((projection.len() as u64).to_be_bytes());
        hasher.update(projection);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
