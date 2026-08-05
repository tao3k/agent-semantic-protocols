//! Hook configuration and managed-profile self-repair receipts.

use agent_semantic_hook::{ClientHookConfig, HookDecision, HookRuntime};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct LoadedHookConfig {
    pub(crate) config: Arc<ClientHookConfig>,
    pub(super) asp_session_policy: super::hook_runtime_agent_session::AspSessionPolicy,
    pub(super) repair_reasons: Vec<String>,
    pub(crate) auto_refresh: Option<String>,
}

static COMPILED_CONFIG_GENERATIONS: OnceLock<Mutex<HashMap<String, Arc<LoadedHookConfig>>>> =
    OnceLock::new();
const MAX_COMPILED_CONFIG_GENERATIONS: usize = 64;

fn append_file_identity(hasher: &mut Sha256, path: &Path) -> Result<(), String> {
    hasher.update(path.as_os_str().as_encoded_bytes());
    match fs::read(path) {
        Ok(bytes) => {
            hasher.update([1]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update([0]),
        Err(error) => {
            return Err(format!(
                "read hook generation input {}: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn compiled_generation_key(
    config_path: &Path,
    project_root: &Path,
    runtime: &HookRuntime,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(b"asp-hook-compiled-generation-v1\0");
    hasher.update(project_root.as_os_str().as_encoded_bytes());
    hasher.update(agent_semantic_config::hook_client_contract_fingerprint().as_bytes());
    let runtime_identity = serde_json::to_vec(runtime)
        .map_err(|error| format!("encode hook runtime generation identity: {error}"))?;
    hasher.update((runtime_identity.len() as u64).to_le_bytes());
    hasher.update(runtime_identity);
    append_file_identity(&mut hasher, config_path)?;
    append_file_identity(
        &mut hasher,
        &agent_semantic_hook::project_agent_config_path(project_root),
    )?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn load_fresh_hook_config(
    config_path: &Path,
    project_root: &Path,
    runtime: &HookRuntime,
) -> Result<(Arc<LoadedHookConfig>, &'static str), String> {
    let initial_key = compiled_generation_key(config_path, project_root, runtime)?;
    let cache = COMPILED_CONFIG_GENERATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let cache = cache
            .lock()
            .map_err(|_| "compiled hook generation cache is poisoned".to_owned())?;
        if let Some(generation) = cache.get(&initial_key) {
            return Ok((Arc::clone(generation), "resident-hit"));
        }
    }

    // Config compilation may atomically refresh managed projections and can
    // re-enter freshness validation.  Never retain the process-wide cache
    // mutex across that work: the cache protects publication, not compilation.
    let generation = Arc::new(load_fresh_hook_config_uncached(
        config_path,
        project_root,
        runtime,
    )?);
    let published_key = compiled_generation_key(config_path, project_root, runtime)?;
    let mut cache = cache
        .lock()
        .map_err(|_| "compiled hook generation cache is poisoned".to_owned())?;
    if let Some(published) = cache.get(&published_key) {
        return Ok((Arc::clone(published), "resident-hit"));
    }
    if cache.len() >= MAX_COMPILED_CONFIG_GENERATIONS {
        let evictable = cache
            .iter()
            .find_map(|(key, value)| (Arc::strong_count(value) == 1).then(|| key.clone()));
        if let Some(key) = evictable {
            cache.remove(&key);
        }
    }
    cache.insert(published_key, Arc::clone(&generation));
    Ok((generation, "compiled"))
}

fn load_fresh_hook_config_uncached(
    config_path: &Path,
    project_root: &Path,
    runtime: &HookRuntime,
) -> Result<LoadedHookConfig, String> {
    let mut config_result =
        agent_semantic_hook::load_client_config_for_project(config_path, project_root);
    let mut session_policy_result = super::load_asp_session_policy(config_path, project_root);
    let mut repair_reasons = Vec::new();
    if let Err(error) = config_result.as_ref() {
        repair_reasons.push(error.clone());
    }
    if let Err(error) = session_policy_result.as_ref()
        && !repair_reasons.contains(error)
    {
        repair_reasons.push(error.clone());
    }
    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    let fingerprint_needs_refresh = config_result
        .as_ref()
        .is_ok_and(|config| config.contract_fingerprint() != Some(expected_fingerprint.as_str()));
    if fingerprint_needs_refresh {
        repair_reasons.push(format!(
            "hook matcher config fingerprint must equal {expected_fingerprint}"
        ));
    }
    let projection_needs_refresh =
        record_language_provider_projection_repair(&config_result, runtime, &mut repair_reasons);
    let needs_refresh = config_result.is_err()
        || session_policy_result.is_err()
        || fingerprint_needs_refresh
        || projection_needs_refresh;
    let mut auto_refresh = None;
    if needs_refresh {
        match super::super::managed_hook_config::materialize(config_path) {
            Ok(status) => {
                auto_refresh = Some(format!("completed:{}", status.as_str()));
                config_result =
                    agent_semantic_hook::load_client_config_for_project(config_path, project_root);
                session_policy_result = super::load_asp_session_policy(config_path, project_root);
            }
            Err(error) => {
                auto_refresh = Some(format!("embedded-current:persistence-failed:{error}"));
                config_result =
                    agent_semantic_hook::load_embedded_client_config_for_project(project_root);
                session_policy_result =
                    super::hook_runtime_agent_session::load_embedded_asp_session_policy(
                        project_root,
                    );
            }
        }
    }
    let refresh_receipt = auto_refresh.as_deref().unwrap_or("not-required");
    let config = config_result.map_err(|error| {
        format!(
            "hook matcher config freshness gate failed for {}: {error}; automatic refresh receipt: {refresh_receipt}",
            config_path.display()
        )
    })?;
    match config.contract_fingerprint() {
        Some(configured) if configured == expected_fingerprint => {}
        Some(configured) => {
            return Err(format!(
                "hook matcher config freshness gate failed for {}: configured fingerprint {configured} does not match binary fingerprint {expected_fingerprint}; automatic refresh receipt: {refresh_receipt}",
                config_path.display()
            ));
        }
        None => {
            return Err(format!(
                "hook matcher config freshness gate failed for {}: contract fingerprint is missing; automatic refresh receipt: {refresh_receipt}",
                config_path.display()
            ));
        }
    }
    let asp_session_policy = session_policy_result.map_err(|error| {
        format!(
            "hook resident config freshness gate failed for {}: {error}; automatic refresh receipt: {refresh_receipt}",
            config_path.display()
        )
    })?;
    Ok(LoadedHookConfig {
        config: Arc::new(config),
        asp_session_policy,
        repair_reasons,
        auto_refresh,
    })
}

pub(super) fn record_language_provider_projection_repair(
    config: &Result<ClientHookConfig, String>,
    runtime: &HookRuntime,
    repair_reasons: &mut Vec<String>,
) -> bool {
    let error = config
        .as_ref()
        .ok()
        .and_then(|config| config.validate_language_provider_projection(runtime).err());
    if let Some(error) = error {
        repair_reasons.push(error);
        true
    } else {
        false
    }
}

pub(crate) fn apply_language_provider_projection(
    config: &ClientHookConfig,
    runtime: &mut HookRuntime,
    config_path: &Path,
    refresh_receipt: &str,
) -> Result<(), String> {
    config
        .apply_language_provider_projection(runtime)
        .map_err(|error| {
            format!(
                "hook language provider projection freshness gate failed for {}: {error}; automatic refresh receipt: {refresh_receipt}",
                config_path.display()
            )
        })
}

pub(super) fn annotate_hook_config_repair(
    decision: &mut HookDecision,
    config_path: &Path,
    repair_reasons: &[String],
    auto_refresh: &str,
) {
    let auto_refresh_completed = auto_refresh.starts_with("completed:");
    let embedded_current = auto_refresh.starts_with("embedded-current:");
    decision.fields.insert(
        "hookConfigStatus".to_string(),
        serde_json::Value::String(
            if auto_refresh_completed {
                "refreshed-by-hook"
            } else if embedded_current {
                "active-from-embedded-authority"
            } else {
                "verified-after-failed-refresh-attempt"
            }
            .to_string(),
        ),
    );
    decision.fields.insert(
        "hookConfigPath".to_string(),
        serde_json::Value::String(config_path.display().to_string()),
    );
    if !repair_reasons.is_empty() {
        decision.fields.insert(
            "hookConfigRepairReasons".to_string(),
            serde_json::Value::Array(
                repair_reasons
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    decision.fields.insert(
        "hookConfigFailurePolicy".to_string(),
        serde_json::Value::String("fail-closed".to_string()),
    );
    decision.fields.insert(
        "hookConfigAutoRefresh".to_string(),
        serde_json::Value::String(auto_refresh.to_string()),
    );
    decision.fields.insert(
        "hookConfigPersistenceStatus".to_string(),
        serde_json::Value::String(
            if auto_refresh_completed {
                "atomically-persisted"
            } else if embedded_current {
                "deferred-read-only-sandbox"
            } else {
                "refresh-not-confirmed"
            }
            .to_string(),
        ),
    );
    let diagnostic = if auto_refresh_completed {
        format!(
            "ASP hook atomically refreshed `{}` before continuing.",
            config_path.display()
        )
    } else if embedded_current {
        format!(
            "ASP hook activated the binary-owned current config in memory because `{}` is not writable in this sandbox; classification continued from embedded authority.",
            config_path.display()
        )
    } else {
        format!(
            "Hook refresh did not report completion, but `{}` passed the required matcher and resident contracts on reload. Automatic refresh receipt: {}",
            config_path.display(),
            auto_refresh
        )
    };
    if decision.message.trim().is_empty() {
        decision.message = diagnostic;
    } else {
        decision.message = format!("{}\n{diagnostic}", decision.message.trim());
    }
}
