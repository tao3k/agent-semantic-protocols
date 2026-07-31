pub(crate) fn project_entries(
    manifest: &agent_semantic_hook::ProviderManifest,
) -> Vec<String> {
    manifest
        .project_resolution()
        .map(|descriptor| descriptor.entry_markers.clone())
        .unwrap_or_default()
}

pub(crate) fn document_extensions(
    manifest: &agent_semantic_hook::ProviderManifest,
) -> Vec<String> {
    manifest
        .document_resolution()
        .map(|descriptor| descriptor.extensions.clone())
        .unwrap_or_default()
}
