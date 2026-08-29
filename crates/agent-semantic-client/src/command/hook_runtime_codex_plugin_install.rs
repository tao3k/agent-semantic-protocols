//! Explicit Codex plugin payload status and publication transaction.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    CODEX_PLUGIN_HOOKS_RELATIVE_PATH, CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH,
    CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, CodexPluginPayloadInspection, CodexPluginPayloadState,
    codex_plugin_cache_root, inspect_codex_plugin_payload, load_codex_plugin_payload_identity,
};
use fs2::FileExt;

use super::{
    ASP_CODEX_PLUGIN_MARKETPLACE_NAME, ASP_CODEX_PLUGIN_NAME, codex_plugin_installed_path,
    codex_plugin_source_root, ensure_codex_plugin_marketplace_registered, global_codex_config_path,
    run_codex_plugin_command,
};

const PLUGIN_ID: &str = "asp-codex-plugin@asp-project";
const PAYLOAD_FILES: [&str; 3] = [
    CODEX_PLUGIN_MANIFEST_RELATIVE_PATH,
    CODEX_PLUGIN_HOOKS_RELATIVE_PATH,
    CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH,
];

pub(in crate::command) fn inspect_codex_plugin_publication(
    project_root: &Path,
    source_root_source: &str,
) -> Result<(), String> {
    let inspection = inspect_current_publication(project_root)?;
    print_inspection(
        "plugin-status",
        &inspection,
        "observed",
        project_root,
        source_root_source,
    );
    Ok(())
}

pub(in crate::command) fn publish_codex_plugin_payload(
    project_root: &Path,
    source_root_source: &str,
) -> Result<(), String> {
    let codex_home = codex_home()?;
    fs::create_dir_all(codex_home.join("plugins")).map_err(|error| {
        format!(
            "failed to create Codex plugin state {}: {error}",
            codex_home.join("plugins").display()
        )
    })?;
    let lock_path = codex_home.join("plugins/asp-plugin-publication.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("failed to open {}: {error}", lock_path.display()))?;
    lock.try_lock_exclusive().map_err(|error| {
        format!(
            "reasonKind=plugin-publication-in-progress lock={} error={error}",
            lock_path.display()
        )
    })?;

    let before = inspect_current_publication(project_root)?;
    if before.state == CodexPluginPayloadState::Current {
        print_inspection(
            "plugin-publish",
            &before,
            "unchanged",
            project_root,
            source_root_source,
        );
        return Ok(());
    }

    let marketplace_root = codex_plugin_source_root(project_root)?;
    let source_root = marketplace_root.join(ASP_CODEX_PLUGIN_NAME);
    let manifest_path = source_root.join(CODEX_PLUGIN_MANIFEST_RELATIVE_PATH);
    let previous_manifest = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let next_manifest = render_cachebusted_manifest(&previous_manifest, &before.source.digest)?;
    write_file_atomically(&manifest_path, &next_manifest)?;

    let publish =
        publish_changed_payload(project_root, &marketplace_root, &source_root, &codex_home);
    let after = match publish {
        Ok(after) => after,
        Err(error) => {
            let restore_source = write_file_atomically(&manifest_path, &previous_manifest);
            let rollback =
                rollback_installed_plugin(project_root, &source_root, &codex_home, &before);
            return Err(format!(
                "reasonKind=plugin-payload-publication-failed error={error} sourceRollback={restore_source:?} installedRollback={rollback:?}"
            ));
        }
    };
    print_inspection(
        "plugin-publish",
        &after,
        "updated",
        project_root,
        source_root_source,
    );
    Ok(())
}

fn publish_changed_payload(
    project_root: &Path,
    marketplace_root: &Path,
    source_root: &Path,
    codex_home: &Path,
) -> Result<CodexPluginPayloadInspection, String> {
    let source = load_codex_plugin_payload_identity(source_root)?;
    ensure_codex_plugin_marketplace_registered(
        project_root,
        marketplace_root,
        Some(codex_home),
        ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
    )?;
    let stdout = run_codex_plugin_command(
        &[
            "plugin".to_owned(),
            "add".to_owned(),
            PLUGIN_ID.to_owned(),
            "--json".to_owned(),
        ],
        project_root,
        Some(codex_home),
    )?;
    let installed_root = codex_plugin_installed_path(&stdout)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            codex_plugin_cache_root(
                codex_home,
                ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
                &source.plugin_name,
                &source.version,
            )
        });
    let inspection = inspect_codex_plugin_payload(source_root, &installed_root)?;
    if inspection.state != CodexPluginPayloadState::Current {
        return Err(format!(
            "Codex accepted plugin add but installed payload is not current: state={} sourceDigest={} installedDigest={} installedRoot={}",
            inspection.state.as_str(),
            inspection.source.digest,
            inspection
                .installed
                .as_ref()
                .map_or("missing", |identity| identity.digest.as_str()),
            inspection.installed_root.display(),
        ));
    }
    Ok(inspection)
}

fn inspect_current_publication(
    project_root: &Path,
) -> Result<CodexPluginPayloadInspection, String> {
    let marketplace_root = codex_plugin_source_root(project_root)?;
    let source_root = marketplace_root.join(ASP_CODEX_PLUGIN_NAME);
    let source = load_codex_plugin_payload_identity(&source_root)?;
    let codex_home = codex_home()?;
    let installed_version = installed_plugin_version(project_root, &codex_home)?;
    let installed_root = codex_plugin_cache_root(
        &codex_home,
        ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
        &source.plugin_name,
        installed_version.as_deref().unwrap_or(&source.version),
    );
    inspect_codex_plugin_payload(&source_root, &installed_root)
}

fn installed_plugin_version(
    project_root: &Path,
    codex_home: &Path,
) -> Result<Option<String>, String> {
    let stdout = run_codex_plugin_command(
        &["plugin".to_owned(), "list".to_owned(), "--json".to_owned()],
        project_root,
        Some(codex_home),
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("invalid codex plugin list JSON: {error}"))?;
    Ok(value
        .get("installed")
        .and_then(serde_json::Value::as_array)
        .and_then(|plugins| {
            plugins.iter().find(|plugin| {
                plugin.get("pluginId").and_then(serde_json::Value::as_str) == Some(PLUGIN_ID)
            })
        })
        .and_then(|plugin| plugin.get("version"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned))
}

fn render_cachebusted_manifest(manifest: &[u8], source_digest: &str) -> Result<Vec<u8>, String> {
    let mut value = serde_json::from_slice::<serde_json::Value>(manifest)
        .map_err(|error| format!("invalid Codex plugin manifest JSON: {error}"))?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| "Codex plugin manifest is missing version".to_owned())?;
    let base = version.split_once('+').map_or(version, |(base, _)| base);
    let digest = source_digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| format!("unexpected plugin payload digest {source_digest}"))?;
    let cachebuster = digest
        .get(..20)
        .ok_or_else(|| format!("plugin payload digest is too short: {source_digest}"))?;
    value["version"] = serde_json::Value::String(format!("{base}+codex.{cachebuster}"));
    let mut bytes = serde_json::to_vec_pretty(&value)
        .map_err(|error| format!("encode cachebusted plugin manifest: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn rollback_installed_plugin(
    project_root: &Path,
    source_root: &Path,
    codex_home: &Path,
    before: &CodexPluginPayloadInspection,
) -> Result<(), String> {
    let Some(previous) = before.installed.as_ref() else {
        return Ok(());
    };
    let previous_root = codex_plugin_cache_root(
        codex_home,
        ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
        &previous.plugin_name,
        &previous.version,
    );
    if !previous_root.is_dir() {
        return Err(format!(
            "previous installed plugin cache is unavailable: {}",
            previous_root.display()
        ));
    }
    let source_snapshot = snapshot_payload(source_root)?;
    let rollback_result = (|| {
        copy_payload(&previous_root, source_root)?;
        run_codex_plugin_command(
            &[
                "plugin".to_owned(),
                "add".to_owned(),
                PLUGIN_ID.to_owned(),
                "--json".to_owned(),
            ],
            project_root,
            Some(codex_home),
        )?;
        Ok(())
    })();
    let restore_result = restore_payload(source_root, &source_snapshot);
    match (rollback_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(rollback), Err(restore)) => Err(format!(
            "installed rollback failed: {rollback}; source restore failed: {restore}"
        )),
    }
}

fn snapshot_payload(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    PAYLOAD_FILES
        .iter()
        .map(|relative| {
            fs::read(root.join(relative))
                .map(|bytes| ((*relative).to_owned(), bytes))
                .map_err(|error| {
                    format!(
                        "failed to snapshot {}: {error}",
                        root.join(relative).display()
                    )
                })
        })
        .collect()
}

fn restore_payload(root: &Path, payload: &[(String, Vec<u8>)]) -> Result<(), String> {
    for (relative, bytes) in payload {
        write_file_atomically(&root.join(relative), bytes)?;
    }
    Ok(())
}

fn copy_payload(from: &Path, to: &Path) -> Result<(), String> {
    let payload = snapshot_payload(from)?;
    restore_payload(to, &payload)
}

fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("plugin payload path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("payload"),
        std::process::id(),
    ));
    let publish = (|| {
        let mut file = fs::File::create(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path).map_err(|error| {
            format!(
                "failed to publish {} to {}: {error}",
                temporary.display(),
                path.display()
            )
        })
    })();
    if publish.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    publish
}

fn codex_home() -> Result<PathBuf, String> {
    global_codex_config_path()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "global Codex config path has no parent".to_owned())
}

fn print_inspection(
    label: &str,
    inspection: &CodexPluginPayloadInspection,
    status: &str,
    source_root: &Path,
    source_root_source: &str,
) {
    println!(
        "[{label}] client=codex pluginScope=global sourceRoot={} sourceRootSource={} state={} reasonKind={} publicationStatus={status} sourceVersion={} sourceDigest={} installedVersion={} installedDigest={} installedRoot={} binaryInstall=not-on-plugin-publication hookGeneration=not-on-plugin-publication runtimeActivation=not-on-plugin-publication",
        source_root.display(),
        source_root_source,
        inspection.state.as_str(),
        inspection.state.as_str(),
        inspection.source.version,
        inspection.source.digest,
        inspection
            .installed
            .as_ref()
            .map_or("missing", |identity| identity.version.as_str()),
        inspection
            .installed
            .as_ref()
            .map_or("missing", |identity| identity.digest.as_str()),
        inspection.installed_root.display(),
    );
}
