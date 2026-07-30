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

#[test]
fn project_resolution_scope_resolves_only_git_candidates() {
    let stdout = br#"{
      "schemaId":"agent.semantic-protocols.provider-project-resolution-response",
      "schemaVersion":"1",
      "languageId":"rust",
      "providerId":"rs-harness",
      "state":"resolved",
      "resolution":{
        "schemaId":"agent.semantic-protocols.project-resolution",
        "schemaVersion":"1",
        "state":"resolved",
        "completeness":"exact",
        "repositoryCandidates":{
          "candidates":[
            {"path":"src/lint_babel.rs"},
            {"path":"src/lib.rs"},
            {"path":"src/generated.rs"},
            {"path":"README.md"}
          ]
        },
        "resolvedSourceScopes":[
          {
            "roots":["src"],
            "extensions":[".rs"],
            "exclusions":[{"prefix":"src/generated.rs","authority":"package-manager"}]
          }
        ]
      }
    }"#;
    let scope = super::project_resolution_scope_from_stdout(
        stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
    )
    .expect("project-resolution response should define the source scope");
    let super::ProviderWorkspaceScope::Supported(packet) = scope else {
        panic!("project-resolution response must not become unsupported");
    };
    assert_eq!(
        packet
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["src/lib.rs", "src/lint_babel.rs"]
    );
}

#[test]
fn legacy_workspace_scope_packet_is_rejected() {
    let error = super::project_resolution_scope_from_stdout(
        br#"{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","schemaVersion":"1","status":"ok","files":[{"path":"src/lint_babel.rs"}]}"#,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
    )
    .expect_err("legacy workspace-scope packets must not be accepted");
    assert!(
        error.contains("decode provider project-resolution response"),
        "{error}"
    );
}
