use agent_semantic_config::HookClientLanguageProviderConfig;

use super::compile_language_provider_snapshot;

fn provider(
    language_id: &str,
    provider_id: &str,
    extensions: &[&str],
) -> HookClientLanguageProviderConfig {
    let manifest = crate::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| {
            manifest.language_id().as_str() == language_id
                && manifest.provider_id().as_str() == provider_id
        })
        .expect("registered provider fixture");
    HookClientLanguageProviderConfig {
        language_id: language_id.to_owned(),
        provider_id: provider_id.to_owned(),
        manifest_digest: crate::provider_manifest_digest(&manifest)
            .expect("provider manifest digest"),
        source_extensions: extensions
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
    }
}

#[test]
fn snapshot_compiles_inactive_document_provider_without_runtime_or_global_state() {
    let snapshot =
        compile_language_provider_snapshot(&[provider("md", "orgize", &[".md", ".markdown"])])
            .expect("compile Markdown hook policy snapshot");
    assert_eq!(snapshot.provider_projections.len(), 1);
    assert_eq!(snapshot.provider_projections[0].language_id.as_str(), "md");
}

#[test]
fn invalid_snapshot_is_rejected_without_mutating_global_state() {
    let invalid = provider("rust", "rs-harness", &[]);
    assert!(compile_language_provider_snapshot(&[invalid]).is_err());
}

#[test]
fn duplicate_provider_identity_is_rejected() {
    let duplicate = provider("rust", "rs-harness", &[".rs"]);
    let error = compile_language_provider_snapshot(&[duplicate.clone(), duplicate])
        .expect_err("duplicate provider identity must fail");
    assert!(error.contains("duplicate provider"));
}
