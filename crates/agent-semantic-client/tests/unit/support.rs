use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub(crate) struct CacheTestLock(std::sync::Mutex<()>);

pub(crate) fn write_hermetic_provider_registry_config(
    root: &Path,
    active_language_id: &str,
    binary: &str,
) {
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    let mut config = String::new();
    for manifest in agent_semantic_hook::builtin_provider_manifests() {
        let language_id = manifest.language_id().as_str();
        if language_id == active_language_id {
            config.push_str(&format!(
                "[providers.{language_id}]\nenabled = true\nbinary = \"{binary}\"\n"
            ));
        } else {
            config.push_str(&format!("[providers.{language_id}]\nenabled = false\n"));
        }
    }
    std::fs::write(config_path, config).expect("write hermetic provider registry config");
}

pub(crate) fn write_hermetic_provider_executable(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("hermetic provider parent"))
        .expect("create hermetic provider parent");
    std::fs::write(path, "#!/bin/sh\nexit 0\n").expect("write hermetic provider executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("hermetic provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)
            .expect("set hermetic provider executable permissions");
    }
}

pub(crate) fn write_hermetic_provider_install_receipt(
    root: &Path,
    active_language_id: &str,
    binary: &Path,
) {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == active_language_id)
        .unwrap_or_else(|| panic!("{active_language_id} provider manifest"));
    let state_paths =
        agent_semantic_runtime::project_state_paths(root).expect("project state paths");
    std::fs::create_dir_all(&state_paths.provider_lock_dir).expect("create provider lock dir");
    let installed_path = std::fs::canonicalize(binary).expect("canonical provider binary");
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed_path)
            .expect("provider content digest");
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed_path)
            .expect("provider metadata digest");
    let receipt = format!(
        "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{}\"\ninstalledPath = {:?}\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\n",
        manifest.provider_id(),
        installed_path.display().to_string(),
        installed_entrypoint_digest,
        installed_entrypoint_metadata_digest,
    );
    std::fs::write(
        state_paths
            .provider_lock_dir
            .join(format!("{active_language_id}.lock.toml")),
        receipt,
    )
    .expect("write hermetic provider install receipt");
}

impl CacheTestLock {
    pub(crate) const fn new() -> Self {
        Self(std::sync::Mutex::new(()))
    }

    pub(crate) fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, std::convert::Infallible> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

pub(crate) static CACHE_TEST_LOCK: CacheTestLock = CacheTestLock::new();

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
        project_resolution: None,
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
