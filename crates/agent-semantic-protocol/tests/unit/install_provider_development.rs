use super::{
    DEFAULT_DEVELOPMENT_PROVIDER_INSTALL_TIMEOUT, development_artifact_is_authorized,
    development_provider_installer_plan,
};
use agent_semantic_hook::ProviderDevelopmentArtifactDomain;

#[test]
fn plain_install_delegates_to_configured_root_installer() {
    let root = std::env::temp_dir().join(format!(
        "asp-dev-installer-plan-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    std::fs::create_dir_all(&root).expect("dev root");
    std::fs::write(root.join("Justfile"), "agent-tools-install-language:\n")
        .expect("development installer");
    let canonical_root = root.canonicalize().expect("canonical dev root");
    let plan =
        development_provider_installer_plan(&canonical_root, "rust", "aarch64-apple-darwin", None)
            .expect("development installer plan");
    assert_eq!(plan.root, canonical_root);
    assert_eq!(
        plan.args,
        vec![
            "exec",
            canonical_root.to_str().expect("utf-8 root"),
            "just",
            "--justfile",
            canonical_root
                .join("Justfile")
                .to_str()
                .expect("utf-8 justfile"),
            "agent-tools-install-language",
            "rust",
            "aarch64-apple-darwin",
            "global",
            "",
        ]
    );
    std::fs::remove_dir_all(root).expect("remove dev root");
}

#[test]
fn artifact_admission_has_only_checkout_and_typed_state_staging_domains() {
    let provider_source_root =
        std::path::Path::new("/checkout/agent-semantic-protocols/languages/python");
    let state_home = std::path::Path::new("/state");
    assert!(development_artifact_is_authorized(
        provider_source_root,
        state_home,
        "py-harness",
        ProviderDevelopmentArtifactDomain::Checkout,
        std::path::Path::new("/checkout/agent-semantic-protocols/languages/python/py-harness")
    ));
    assert!(development_artifact_is_authorized(
        provider_source_root,
        state_home,
        "py-harness",
        ProviderDevelopmentArtifactDomain::StateHomeProviderStaging,
        std::path::Path::new("/state/runtime/provider-artifacts/py-harness/develop/py-harness")
    ));
    assert!(!development_artifact_is_authorized(
        provider_source_root,
        state_home,
        "py-harness",
        ProviderDevelopmentArtifactDomain::StateHomeProviderStaging,
        std::path::Path::new("/state/runtime/bin/py-harness")
    ));
    assert!(!development_artifact_is_authorized(
        provider_source_root,
        state_home,
        "py-harness",
        ProviderDevelopmentArtifactDomain::StateHomeProviderStaging,
        std::path::Path::new("/state/runtime/provider-artifacts/ts-harness/develop/py-harness")
    ));
    assert!(!development_artifact_is_authorized(
        provider_source_root,
        state_home,
        "py-harness",
        ProviderDevelopmentArtifactDomain::Checkout,
        std::path::Path::new("/checkout/agent-semantic-protocols/languages/typescript/py-harness")
    ));
}

#[test]
fn development_installer_default_timeout_is_bounded() {
    assert_eq!(
        DEFAULT_DEVELOPMENT_PROVIDER_INSTALL_TIMEOUT,
        std::time::Duration::from_secs(15 * 60)
    );
}
