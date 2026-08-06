use agent_semantic_config::HookClientLanguageProviderConfig;

use super::{
    active_policy_snapshot, active_provider_projections, compile_language_provider_snapshot,
    publish_language_provider_snapshot,
};

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
fn snapshot_projects_inactive_document_provider_without_runtime_server() {
    let project = "hook-policy-kernel-markdown";
    publish_language_provider_snapshot(project, &[provider("md", "orgize", &[".md", ".markdown"])])
        .expect("publish Markdown hook policy snapshot");
    let projections = active_provider_projections(project).expect("active projections");
    assert_eq!(projections.len(), 1);
    assert_eq!(projections[0].language_id.as_str(), "md");
}

#[test]
fn invalid_refresh_preserves_last_known_good_snapshot() {
    let project = "hook-policy-kernel-last-known-good";
    let active =
        publish_language_provider_snapshot(project, &[provider("rust", "rs-harness", &[".rs"])])
            .expect("publish admitted snapshot");
    let invalid = provider("rust", "rs-harness", &[]);
    assert!(publish_language_provider_snapshot(project, &[invalid]).is_err());
    let retained = active_policy_snapshot(project).expect("retain last known good snapshot");
    assert_eq!(retained.generation_digest, active.generation_digest);
}

#[test]
fn duplicate_provider_identity_is_rejected() {
    let duplicate = provider("rust", "rs-harness", &[".rs"]);
    let error = compile_language_provider_snapshot(&[duplicate.clone(), duplicate])
        .expect_err("duplicate provider identity must fail");
    assert!(error.contains("duplicate provider"));
}
