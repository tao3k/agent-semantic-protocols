use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

static ASP_STATE_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

pub(crate) struct IsolatedAspStateHome {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Option<OsString>,
}

impl IsolatedAspStateHome {
    pub(crate) fn activate(root: &Path) -> Self {
        let guard = ASP_STATE_HOME_ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

pub(crate) fn init_durable_repo(root: &Path, label: &str) {
    let init = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["init", "--quiet"])
        .output()
        .expect("run git init");
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let remote = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "remote",
            "add",
            "origin",
            &format!("https://example.invalid/asp/{label}.git"),
        ])
        .output()
        .expect("add canonical remote");
    assert!(
        remote.status.success(),
        "git remote add failed: {}",
        String::from_utf8_lossy(&remote.stderr)
    );
}
pub(super) fn runtime_provider() -> crate::RuntimeProvider {
    crate::RuntimeProvider {
        registration_digest: "sha256:test".to_string(),
        namespace: "agent.semantic-protocols.languages.rust".to_string(),
        language_id: crate::LanguageId::from("rust"),
        provider_id: crate::ProviderId::from("asp-rust"),
        binary: "/test/asp-rust".to_string(),
        package_roots: vec![".".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        source_inventory_capabilities: crate::ProviderSourceInventoryCapabilities {
            project_resolution: Some(crate::ProviderProjectInventoryCapability {
                entry_markers: vec!["Cargo.toml".to_string()],
            }),
            document_resolution: None,
        },
        search_capabilities: serde_json::from_value(serde_json::json!({
            "ownerItems": true,
            "semanticFacts": true,
            "dependencyTopology": true,
            "dependencyTopologyMetadata": true
        }))
        .expect("search capabilities"),
        query_pack_descriptor: serde_json::from_value(serde_json::json!({
            "descriptorId": "rust.search",
            "descriptorVersion": "1",
            "languageId": "rust",
            "termRoleOverrides": [],
            "recipes": []
        }))
        .expect("query pack descriptor"),
        semantic_facts_descriptor: None,
        runtime_operations: Vec::new(),
    }
}
