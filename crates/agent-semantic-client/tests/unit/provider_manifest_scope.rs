// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(crate) fn project_entries(manifest: &agent_semantic_hook::ProviderManifest) -> Vec<String> {
    manifest
        .project_resolution()
        .map(|descriptor| descriptor.entry_markers.clone())
        .unwrap_or_default()
}

pub(crate) fn document_extensions(manifest: &agent_semantic_hook::ProviderManifest) -> Vec<String> {
    manifest
        .document_resolution()
        .map(|descriptor| descriptor.extensions.clone())
        .unwrap_or_default()
}
