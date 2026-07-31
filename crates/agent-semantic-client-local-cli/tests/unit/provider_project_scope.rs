use std::path::Path;

use super::{
    ProviderProjectScopeFile, ProviderProjectScopePacket, provider_project_scope_files_from_packet,
};

#[test]
fn provider_project_scope_packet_is_the_only_file_scope_input() {
    let scope = provider_project_scope_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderProjectScopePacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            files: Vec::new(),
        },
    );
    assert!(scope.is_empty());
}

#[test]
fn provider_project_scope_packet_does_not_materialize_missing_files() {
    let files = provider_project_scope_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderProjectScopePacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            files: vec![ProviderProjectScopeFile {
                path: "this-file-does-not-exist.rs".into(),
                language_id: "rust".into(),
                provider_id: "rust-lang-project-harness".into(),
            }],
        },
    );
    assert!(files.is_empty());
}

#[tokio::test]
async fn async_scope_file_admission_is_deterministic_and_ignores_missing_files() {
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "asp-provider-scope-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    tokio::fs::create_dir_all(root.join("src"))
        .await
        .expect("create provider scope fixture");
    tokio::fs::write(root.join("src/z.rs"), b"fn z() {}\n")
        .await
        .expect("write z owner");
    tokio::fs::write(root.join("src/a.rs"), b"fn a() {}\n")
        .await
        .expect("write a owner");
    let packet = ProviderProjectScopePacket {
        language_id: "rust".into(),
        provider_id: "rs-harness".into(),
        files: ["src/z.rs", "missing.rs", "src/a.rs"]
            .into_iter()
            .map(|path| ProviderProjectScopeFile {
                path: path.into(),
                language_id: "rust".into(),
                provider_id: "rs-harness".into(),
            })
            .collect(),
    };

    let admitted = super::provider_project_scope_files_from_packet_async(&root, &root, packet)
        .await
        .expect("admit async scope files");
    assert_eq!(
        admitted
            .iter()
            .map(|file| {
                file.path
                    .strip_prefix(&root)
                    .expect("fixture-relative owner")
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec!["src/a.rs".to_owned(), "src/z.rs".to_owned()]
    );
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove provider scope fixture");
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
          ],
          "policyExclusions":[]
        },
        "resolvedSourceScopes":[
          {
            "roots":["src"],
            "extensions":[".rs"],
            "includeAuthority":"package-manager",
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
    let super::ProviderProjectScope::Supported(packet) = scope else {
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
fn project_resolution_scope_applies_typed_policy_exclusions_once() {
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
            {"path":"src/lib.rs"},
            {"path":"src/generated.rs"}
          ],
          "policyExclusions":[
            {"path":"src/generated.rs","authority":"user-policy"}
          ]
        },
        "resolvedSourceScopes":[
          {
            "roots":["src"],
            "extensions":[".rs"],
            "includeAuthority":"package-manager",
            "exclusions":[]
          }
        ]
      }
    }"#;
    let scope = super::project_resolution_scope_from_stdout(
        stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
    )
    .expect("ASP policy should filter an implicit package-manager scope");
    let super::ProviderProjectScope::Supported(packet) = scope else {
        panic!("project-resolution response must not become unsupported");
    };
    assert_eq!(
        packet
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["src/lib.rs"]
    );
}

#[test]
fn explicit_package_target_and_policy_exclusion_fail_closed() {
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
          "candidates":[{"path":"generated/lib.rs"}],
          "policyExclusions":[
            {"path":"generated/lib.rs","authority":"user-policy"}
          ]
        },
        "resolvedSourceScopes":[
          {
            "roots":["generated"],
            "extensions":[".rs"],
            "includeAuthority":"manifest-explicit",
            "exclusions":[]
          }
        ]
      }
    }"#;
    let error = super::project_resolution_scope_from_stdout(
        stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
    )
    .expect_err("explicit package target exclusion must fail closed");

    assert!(error.contains("project-scope-conflict"), "{error}");
    assert!(error.contains("path=generated/lib.rs"), "{error}");
    assert!(error.contains("excludeAuthority=user-policy"), "{error}");
}
