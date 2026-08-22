use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn provider_install_receipt_matches_artifact(
    receipt: &agent_semantic_runtime::ProviderInstallReceipt,
    installed_path: &Path,
) -> Result<bool, String> {
    let expected_path = receipt
        .installed_path
        .canonicalize()
        .unwrap_or_else(|_| receipt.installed_path.clone());
    let actual_path = installed_path
        .canonicalize()
        .unwrap_or_else(|_| installed_path.to_path_buf());
    if expected_path != actual_path {
        return Ok(false);
    }
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(installed_path)?;
    Ok(metadata_digest.as_str() == receipt.installed_entrypoint_metadata_digest)
}

pub(super) fn read_provider_install_receipt(
    language_id: &str,
    provider_lock_dir: &Path,
) -> Result<agent_semantic_runtime::ProviderInstallReceipt, String> {
    let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    let contents = fs::read_to_string(&lock_path)
        .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))?;
    let lock: toml::Value = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", lock_path.display()))?;
    let table = lock
        .as_table()
        .ok_or_else(|| format!("provider lock is not a TOML table: {}", lock_path.display()))?;
    let field = |name: &str| -> Result<&str, String> {
        table
            .get(name)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "provider lock lacks required field `{name}`: {}",
                    lock_path.display()
                )
            })
    };
    if field("schemaId")? != "asp.provider-install-lock.v1" {
        return Err(format!(
            "provider lock schema mismatch: {}",
            lock_path.display()
        ));
    }
    if field("language")? != language_id {
        return Err(format!(
            "provider lock language mismatch: expected={language_id} actual={} lock={}",
            field("language")?,
            lock_path.display()
        ));
    }
    Ok(agent_semantic_runtime::ProviderInstallReceipt {
        language_id: language_id.to_owned(),
        provider_id: field("provider")?.to_owned(),
        installed_path: PathBuf::from(field("installedPath")?),
        installed_entrypoint_digest: field("installedEntrypointDigest")?.to_owned(),
        installed_entrypoint_metadata_digest: field("installedEntrypointMetadataDigest")?
            .to_owned(),
        execution_command_digest: field("executionCommandDigest")?.to_owned(),
    })
}

pub(super) fn reconcile_provider_install_receipt(
    language_id: &str,
    project_root: &Path,
) -> Result<(), String> {
    let provider_lock_dir =
        agent_semantic_runtime::project_state_paths(project_root)?.provider_lock_dir;
    reconcile_provider_install_receipt_in_lock_dir(language_id, &provider_lock_dir, true)?;
    Ok(())
}

pub(super) fn reconcile_provider_install_receipt_in_lock_dir(
    language_id: &str,
    provider_lock_dir: &Path,
    emit_receipt: bool,
) -> Result<bool, String> {
    let lock_path = provider_lock_dir.join(format!("{language_id}.lock.toml"));
    let contents = fs::read_to_string(&lock_path)
        .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))?;
    let mut lock: toml::Value = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", lock_path.display()))?;
    let table = lock
        .as_table_mut()
        .ok_or_else(|| format!("provider lock is not a TOML table: {}", lock_path.display()))?;
    let (current_provider_id, installed_path) = {
        let field = |name: &str| -> Result<&str, String> {
            table
                .get(name)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "provider lock lacks required field `{name}`: {}",
                        lock_path.display()
                    )
                })
        };
        if field("schemaId")? != "asp.provider-install-lock.v1" {
            return Err(format!(
                "provider lock schema mismatch: {}",
                lock_path.display()
            ));
        }
        if field("language")? != language_id {
            return Err(format!(
                "provider lock language mismatch: expected={language_id} actual={} lock={}",
                field("language")?,
                lock_path.display()
            ));
        }
        (
            field("provider")?.to_string(),
            PathBuf::from(field("installedPath")?),
        )
    };
    let provider_id = agent_semantic_hook::registered_provider_id_v1(language_id)
        .ok_or_else(|| format!("no registered provider identity for language `{language_id}`"))?;
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed_path)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed_path)?;
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &[installed_path.to_string_lossy().to_string()],
        &installed_entrypoint_digest,
    )?;
    let installed_size_bytes = fs::metadata(&installed_path)
        .map_err(|error| {
            format!(
                "failed to inspect installed provider {}: {error}",
                installed_path.display()
            )
        })?
        .len();
    let changed = current_provider_id != provider_id
        || table
            .get("installedEntrypointDigest")
            .and_then(toml::Value::as_str)
            != Some(installed_entrypoint_digest.as_str())
        || table
            .get("installedEntrypointMetadataDigest")
            .and_then(toml::Value::as_str)
            != Some(installed_entrypoint_metadata_digest.as_str())
        || table
            .get("executionCommandDigest")
            .and_then(toml::Value::as_str)
            != Some(execution_command_digest.as_str());
    table.insert(
        "provider".to_string(),
        toml::Value::String(provider_id.clone()),
    );
    table.insert(
        "installedEntrypointDigest".to_string(),
        toml::Value::String(installed_entrypoint_digest.clone()),
    );
    table.insert(
        "installedEntrypointMetadataDigest".to_string(),
        toml::Value::String(installed_entrypoint_metadata_digest.clone()),
    );
    table.insert(
        "executionCommandDigest".to_string(),
        toml::Value::String(execution_command_digest.clone()),
    );
    if changed {
        let reconciled = toml::to_string_pretty(&lock)
            .map_err(|error| format!("failed to encode {}: {error}", lock_path.display()))?;
        atomic_write_provider_lock(&lock_path, reconciled.as_bytes())?;
    }
    if emit_receipt {
        println!(
            "[asp-install] provider={} language={} installMode=reconcile-receipt receiptStatus={} installedPath={} installedEntrypointDigest={} installedEntrypointMetadataDigest={} executionCommandDigest={} contentBytesRead={} lock={} switch=atomic",
            provider_id,
            language_id,
            if changed { "updated" } else { "current" },
            installed_path.display(),
            installed_entrypoint_digest,
            installed_entrypoint_metadata_digest,
            execution_command_digest,
            installed_size_bytes,
            lock_path.display(),
        );
    }
    Ok(changed)
}

pub(super) fn atomic_write_provider_lock(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("provider lock has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("provider lock has invalid filename: {}", path.display()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock predates UNIX epoch: {error}"))?
        .as_nanos();
    let temporary = parent.join(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()));
    fs::write(&temporary, contents)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to atomically replace {} from {}: {error}",
            path.display(),
            temporary.display()
        )
    })
}

#[cfg(all(test, unix))]
#[path = "../../tests/unit/provider_install_receipt_reconciliation.rs"]
mod provider_install_receipt_reconciliation_tests;
