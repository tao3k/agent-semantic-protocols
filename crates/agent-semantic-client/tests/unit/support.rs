use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub(crate) static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

pub(crate) fn lookup_current_source_index_for_language(
    project_root: &std::path::Path,
    language_id: Option<&agent_semantic_client_core::LanguageId>,
    query: &str,
    limit: u32,
) -> Result<crate::source_index::SourceIndexLookupResult, String> {
    let snapshot = crate::source_index::current_source_index_snapshot(project_root)?;
    crate::source_index::lookup_source_index_for_language(
        project_root,
        &snapshot.source_snapshot,
        language_id,
        query,
        limit,
    )
}

pub(crate) fn v2_cache_root(workspace_state_root: &Path) -> PathBuf {
    workspace_state_root.join("live").join("client")
}

pub(crate) fn artifacts_root_from_cache_root(cache_root: &Path) -> PathBuf {
    let live_dir = cache_root.parent().expect("cache root live dir");
    assert_eq!(
        cache_root.file_name().and_then(|name| name.to_str()),
        Some("client")
    );
    assert_eq!(
        live_dir.file_name().and_then(|name| name.to_str()),
        Some("live")
    );
    live_dir
        .parent()
        .expect("cache root workspace dir")
        .join("artifacts")
}
pub(super) fn resolved_provider(language_id: &str) -> agent_semantic_client_core::ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == language_id)
        .unwrap_or_else(|| panic!("builtin provider manifest for {language_id}"));
    let manifest_digest = agent_semantic_hook::provider_manifest_digest(&manifest)
        .unwrap_or_else(|error| panic!("{language_id} manifest digest: {error}"));
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let routes = agent_semantic_hook::materialize_provider_routes(&manifest)
        .unwrap_or_else(|error| panic!("{language_id} provider routes: {error}"));
    let current_exe =
        std::env::current_exe().unwrap_or_else(|error| panic!("current test executable: {error}"));
    let verified_executable_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(&current_exe)
            .unwrap_or_else(|error| panic!("{language_id} executable digest: {error}"));
    let provider_command_prefix = vec![current_exe.to_string_lossy().into_owned()];
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &provider_command_prefix,
        &verified_executable_artifact_digest,
    )
    .unwrap_or_else(|error| panic!("{language_id} execution command digest: {error}"));
    let provider = agent_semantic_hook::ActivatedProvider {
        manifest_id: manifest.manifest_id().to_owned(),
        manifest_digest,
        language_id: manifest.language_id().clone(),
        provider_id: manifest.provider_id().clone(),
        binary: manifest.binary().to_owned(),
        execution: manifest.execution(),
        provider_command_prefix,
        execution_command_digest,
        namespace: manifest.namespace().to_owned(),
        package_roots: vec![".".to_string()],
        source_extensions: manifest.source().default_extensions.clone(),
        config_files: manifest.source().default_config_files.clone(),
        source_roots: manifest.source().default_source_roots.clone(),
        ignored_path_prefixes: manifest.source().default_ignored_path_prefixes.clone(),
        search_capabilities: manifest.search_capabilities().clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
        query_pack_descriptor: manifest.query_pack_descriptor().clone(),
        semantic_registry_digest,
        policy: manifest.policy().clone(),
        routes,
    };

    agent_semantic_client_core::ResolvedProvider::try_from(&provider)
        .unwrap_or_else(|error| panic!("canonical activated {language_id} provider: {error}"))
}
