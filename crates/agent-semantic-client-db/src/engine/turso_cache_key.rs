// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_core::CacheGenerationId;

pub(super) fn active_cache_lookup_key(
    project_root: &str,
    language_id: &str,
    provider_id: &str,
    export_method: &str,
    request_fingerprint: &str,
) -> String {
    canonical_active_cache_key(
        "lookup-v1",
        [
            project_root,
            language_id,
            provider_id,
            export_method,
            request_fingerprint,
        ],
    )
}

pub(super) fn active_cache_generation_key(
    project_root: &str,
    language_id: &str,
    provider_id: &str,
    export_method: &str,
    generation_id: &CacheGenerationId,
) -> String {
    canonical_active_cache_key(
        "generation-v1",
        [
            project_root,
            language_id,
            provider_id,
            export_method,
            generation_id.as_str(),
        ],
    )
}

fn canonical_active_cache_key<const N: usize>(kind: &str, components: [&str; N]) -> String {
    let mut key = String::from(kind);
    for component in components {
        key.push('|');
        key.push_str(&component.len().to_string());
        key.push(':');
        key.push_str(component);
    }
    key
}
