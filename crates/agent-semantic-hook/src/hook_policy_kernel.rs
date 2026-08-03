use agent_semantic_config::HookClientLanguageProviderConfig;
use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

pub(crate) const HOOK_POLICY_KERNEL_VERSION: &str = "hook-enforcement-kernel-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HookPolicySnapshot {
    pub kernel_version: &'static str,
    pub generation_digest: String,
    pub language_providers: Vec<HookClientLanguageProviderConfig>,
}

static ACTIVE_POLICY_SNAPSHOTS: OnceLock<RwLock<HashMap<String, Arc<HookPolicySnapshot>>>> =
    OnceLock::new();

fn snapshot_store() -> &'static RwLock<HashMap<String, Arc<HookPolicySnapshot>>> {
    ACTIVE_POLICY_SNAPSHOTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn compile_language_provider_snapshot(
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<HookPolicySnapshot, String> {
    if language_providers.is_empty() {
        return Err("hook policy snapshot requires at least one language provider".to_owned());
    }
    let mut providers = language_providers.to_vec();
    providers.sort_by(|left, right| {
        (&left.language_id, &left.provider_id).cmp(&(&right.language_id, &right.provider_id))
    });
    let mut identities = BTreeSet::new();
    for provider in &providers {
        let identity = (provider.language_id.as_str(), provider.provider_id.as_str());
        if provider.language_id.trim().is_empty()
            || provider.provider_id.trim().is_empty()
            || provider.manifest_digest.trim().is_empty()
        {
            return Err("hook policy snapshot provider identity is incomplete".to_owned());
        }
        if !identities.insert(identity) {
            return Err(format!(
                "hook policy snapshot contains duplicate provider `{}/{}`",
                provider.language_id, provider.provider_id
            ));
        }
        if provider.source_extensions.is_empty() {
            return Err(format!(
                "hook policy snapshot provider `{}/{}` has no source extensions",
                provider.language_id, provider.provider_id
            ));
        }
        let mut extensions = BTreeSet::new();
        for extension in &provider.source_extensions {
            if !extension.starts_with('.') || extension.len() < 2 {
                return Err(format!(
                    "hook policy snapshot provider `{}/{}` has invalid extension `{extension}`",
                    provider.language_id, provider.provider_id
                ));
            }
            if !extensions.insert(extension.as_str()) {
                return Err(format!(
                    "hook policy snapshot provider `{}/{}` repeats extension `{extension}`",
                    provider.language_id, provider.provider_id
                ));
            }
        }
    }
    let canonical = serde_json::to_vec(&providers)
        .map_err(|error| format!("serialize hook policy language providers: {error}"))?;
    let digest = canonical_content_digest_v1(
        b"agent.semantic-protocols.hook-policy-language-providers.v1",
        &[HOOK_POLICY_KERNEL_VERSION.as_bytes(), canonical.as_slice()],
    );
    Ok(HookPolicySnapshot {
        kernel_version: HOOK_POLICY_KERNEL_VERSION,
        generation_digest: format!("blake3-256:{}", digest.as_str()),
        language_providers: providers,
    })
}

pub(crate) fn publish_language_provider_snapshot(
    project_root: &str,
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<Arc<HookPolicySnapshot>, String> {
    let candidate = Arc::new(compile_language_provider_snapshot(language_providers)?);
    let mut snapshots = snapshot_store()
        .write()
        .map_err(|_| "hook policy snapshot store is poisoned".to_owned())?;
    if let Some(active) = snapshots.get(project_root)
        && active.generation_digest == candidate.generation_digest
    {
        return Ok(Arc::clone(active));
    }
    snapshots.insert(project_root.to_owned(), Arc::clone(&candidate));
    Ok(candidate)
}

pub(crate) fn active_policy_snapshot(project_root: &str) -> Option<Arc<HookPolicySnapshot>> {
    snapshot_store()
        .read()
        .ok()
        .and_then(|snapshots| snapshots.get(project_root).cloned())
}

pub(crate) fn snapshot_supports_source_file(project_root: &str, path: &Path) -> Option<bool> {
    let snapshot = active_policy_snapshot(project_root)?;
    debug_assert_eq!(snapshot.kernel_version, HOOK_POLICY_KERNEL_VERSION);
    Some(snapshot.language_providers.iter().any(|provider| {
        agent_semantic_config::source_extension::source_extensions_support_file(
            &provider.source_extensions,
            path,
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(
        language_id: &str,
        provider_id: &str,
        extensions: &[&str],
    ) -> HookClientLanguageProviderConfig {
        HookClientLanguageProviderConfig {
            language_id: language_id.to_owned(),
            provider_id: provider_id.to_owned(),
            manifest_digest: format!("manifest-{language_id}-{provider_id}"),
            source_extensions: extensions
                .iter()
                .map(|extension| (*extension).to_owned())
                .collect(),
        }
    }

    #[test]
    fn snapshot_recognizes_configured_provider_without_runtime_server() {
        let project = "hook-policy-kernel-typescript";
        publish_language_provider_snapshot(
            project,
            &[provider("typescript", "ts-harness", &[".ts", ".tsx"])],
        )
        .expect("publish TypeScript hook policy snapshot");
        assert_eq!(
            snapshot_supports_source_file(project, Path::new("src/app.ts")),
            Some(true)
        );
    }

    #[test]
    fn invalid_refresh_preserves_last_known_good_snapshot() {
        let project = "hook-policy-kernel-last-known-good";
        let active = publish_language_provider_snapshot(
            project,
            &[provider("rust", "rs-harness", &[".rs"])],
        )
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
}
