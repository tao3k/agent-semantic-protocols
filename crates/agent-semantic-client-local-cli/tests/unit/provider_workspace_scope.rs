use std::path::Path;

use super::{
    ProviderWorkspaceScopeFile, ProviderWorkspaceScopePacket,
    provider_workspace_scope_files_from_packet,
};

#[test]
fn provider_workspace_scope_packet_is_the_only_file_scope_input() {
    let scope = provider_workspace_scope_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderWorkspaceScopePacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            files: Vec::new(),
        },
    );
    assert!(scope.is_empty());
}

#[test]
fn provider_workspace_scope_packet_does_not_materialize_missing_files() {
    let files = provider_workspace_scope_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderWorkspaceScopePacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            files: vec![ProviderWorkspaceScopeFile {
                path: "this-file-does-not-exist.rs".into(),
                language_id: "rust".into(),
                provider_id: "rust-lang-project-harness".into(),
            }],
        },
    );
    assert!(files.is_empty());
}
