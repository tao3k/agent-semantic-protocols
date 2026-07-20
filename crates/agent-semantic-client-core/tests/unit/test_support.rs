use std::{ffi::OsString, path::Path, sync::Mutex};

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct IsolatedAspStateHome {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl IsolatedAspStateHome {
    pub(crate) fn activate(root: &Path) -> Self {
        let guard = ASP_STATE_HOME_ENV_LOCK
            .lock()
            .expect("ASP_STATE_HOME env lock");
        let previous = std::env::var_os("ASP_STATE_HOME");
        let state_home = root.join(".agent-semantic-protocols-test-state");
        unsafe {
            std::env::set_var("ASP_STATE_HOME", &state_home);
        }
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for IsolatedAspStateHome {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var("ASP_STATE_HOME", previous);
            } else {
                std::env::remove_var("ASP_STATE_HOME");
            }
        }
    }
}
pub(super) fn resolved_provider() -> crate::ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == "rust")
        .expect("builtin Rust provider manifest");
    let manifest_digest =
        agent_semantic_hook::provider_manifest_digest(&manifest).expect("Rust manifest digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("Rust provider routes");
    let provider = agent_semantic_hook::ActivatedProvider {
        manifest_id: manifest.manifest_id,
        manifest_digest,
        language_id: manifest.language_id,
        provider_id: manifest.provider_id,
        binary: manifest.binary,
        execution: manifest.execution,
        provider_command_prefix: vec!["rs-harness".to_string()],
        namespace: manifest.namespace,
        package_roots: vec![".".to_string()],
        source_extensions: manifest.source.default_extensions,
        config_files: manifest.source.default_config_files,
        source_roots: manifest.source.default_source_roots,
        ignored_path_prefixes: manifest.source.default_ignored_path_prefixes,
        search_capabilities: manifest.search_capabilities,
        semantic_facts_descriptor: manifest.semantic_facts_descriptor,
        query_pack_descriptor: manifest.query_pack_descriptor,
        semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
        policy: manifest.policy,
        routes,
    };

    crate::ResolvedProvider::try_from(&provider).expect("canonical activated Rust provider")
}
