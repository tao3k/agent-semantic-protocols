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

pub(crate) async fn lookup_current_source_index_for_language(
    project_root: &std::path::Path,
    language_id: Option<&agent_semantic_client_core::LanguageId>,
    query: &str,
    limit: u32,
) -> Result<crate::source_index::SourceIndexLookupResult, String> {
    let supervisor = agent_semantic_provider_transport::ProviderProcessSupervisor::default();
    let snapshot =
        crate::source_index::current_source_index_snapshot(&supervisor, project_root).await?;
    supervisor.shutdown().await;
    crate::source_index::lookup_source_index_for_language(
        project_root,
        &snapshot.source_snapshot,
        language_id,
        query,
        limit,
    )
    .await
}

pub(crate) fn owner_backed_temp_root(label: &str) -> PathBuf {
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    static FIXTURE_BASE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

    let fixture_base = FIXTURE_BASE.get_or_init(|| {
        let repository = gix::discover(env!("CARGO_MANIFEST_DIR"))
            .expect("discover the owner-backed client test repository with Gix");
        let worktree = repository
            .worktree()
            .expect("client tests require a non-bare owner checkout");
        let base = worktree.base().join("target/asp-live-project-fixtures");
        std::fs::create_dir_all(&base).expect("create owner-backed client fixture root");
        base
    });
    let fixture_id = FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = fixture_base.join(format!(
        "agent-semantic-client-{label}-{}-{fixture_id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create isolated owner-backed client fixture");
    gix::init(&root).expect("initialize owner-backed client fixture with Gix");
    root
}
