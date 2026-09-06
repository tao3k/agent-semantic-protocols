//! Explicit Codex plugin payload status and publication transaction.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use agent_semantic_config::CODEX_PLUGIN_HOOKS_RELATIVE_PATH;
use agent_semantic_config::CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH;
use agent_semantic_config::CODEX_PLUGIN_MANIFEST_RELATIVE_PATH;
use agent_semantic_config::CodexPluginPayloadInspection;
use agent_semantic_config::CodexPluginPayloadState;
use agent_semantic_config::codex_plugin_cache_root;
use agent_semantic_config::inspect_codex_plugin_payload;
use agent_semantic_config::load_codex_plugin_payload_identity;
use fs2::FileExt;

use super::ASP_CODEX_PLUGIN_MARKETPLACE_NAME;
use super::ASP_CODEX_PLUGIN_NAME;
use super::codex_plugin_source_root;
use super::ensure_codex_plugin_marketplace_registered;
use super::global_codex_config_path;
use super::run_codex_plugin_command;

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
    // Policy is a State Home artifact, not part of the fixed Codex plugin
    // payload. Always synchronize it first; a current hooks.json must never
    // force a cache-busted plugin reinstall just to publish config.toml.
    let protocol_home = agent_semantic_runtime::project_protocol_home_path(project_root)?;
    let hook_config_status =
        crate::command::install_binary_config_admission::publish_embedded_hook_config(
            &protocol_home,
        )?;
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
        if !launcher_is_executable(&before.installed_root)? {
            // The payload bytes are still Codex-trusted, but a cache write may
            // have lost the launcher mode. Codex renders that as a bare
            // `hook exited with code 126`; repair this metadata-only cache
            // defect atomically without changing the reviewed payload.
            let marketplace_root = codex_plugin_source_root(project_root)?;
            let source_root = marketplace_root.join(ASP_CODEX_PLUGIN_NAME);
            let repaired = publish_direct_global_cache(&source_root, &codex_home)?;
            if !launcher_is_executable(&repaired.installed_root)? {
                return Err(format!(
                    "reasonKind=plugin-launcher-not-executable installedRoot={}",
                    repaired.installed_root.display()
                ));
            }
            print_inspection(
                "plugin-publish",
                &repaired,
                "repaired-launcher-mode",
                project_root,
                source_root_source,
            );
            return Ok(());
        }
        println!(
            "[hook-config-sync] path={} status={} pluginPayload=current",
            protocol_home.join("hooks/config.toml").display(),
            hook_config_status
        );
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
    let (after, transport) =
        match publish_changed_payload(project_root, &marketplace_root, &source_root, &codex_home) {
            Ok(inspection) => (inspection, "codex-plugin-cli"),
            Err(error) if native_codex_cli_unavailable(&error) => (
                publish_direct_global_cache(&source_root, &codex_home)?,
                "direct-global-cache",
            ),
            Err(error) => {
                return Err(format!(
                    "reasonKind=plugin-payload-publication-failed error={error}"
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
    println!("[plugin-publish] publicationTransport={transport} hookTrust=codex-review-required");
    Ok(())
}

fn launcher_is_executable(installed_root: &Path) -> Result<bool, String> {
    let launcher = installed_root.join(CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH);
    let metadata = fs::metadata(&launcher)
        .map_err(|error| format!("failed to stat {}: {error}", launcher.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        return Ok(metadata.permissions().mode() & 0o111 != 0);
    }
    #[cfg(not(unix))]
    {
        Ok(metadata.is_file())
    }
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
    let _stdout = run_codex_plugin_command(
        &[
            "plugin".to_owned(),
            "add".to_owned(),
            PLUGIN_ID.to_owned(),
            "--json".to_owned(),
        ],
        project_root,
        Some(codex_home),
    )?;
    let installed_root = codex_plugin_cache_root(
        codex_home,
        ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
        &source.plugin_name,
        &source.version,
    );
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

/// Publish directly to Codex's deterministic global cache only when its native
/// CLI cannot be launched. Codex remains the trust authority for changed hooks.
fn publish_direct_global_cache(
    source_root: &Path,
    codex_home: &Path,
) -> Result<CodexPluginPayloadInspection, String> {
    let source = load_codex_plugin_payload_identity(source_root)?;
    let installed_root = codex_plugin_cache_root(
        codex_home,
        ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
        &source.plugin_name,
        &source.version,
    );
    for relative in PAYLOAD_FILES {
        let source_path = source_root.join(relative);
        let bytes = fs::read(&source_path)
            .map_err(|error| format!("failed to read plugin source payload {relative}: {error}"))?;
        write_file_atomically(&source_path, &installed_root.join(relative), &bytes)?;
    }
    let inspection = inspect_codex_plugin_payload(source_root, &installed_root)?;
    if inspection.state != CodexPluginPayloadState::Current {
        return Err(format!(
            "direct global plugin cache publication is not current: state={} sourceDigest={} installedRoot={}",
            inspection.state.as_str(),
            inspection.source.digest,
            inspection.installed_root.display()
        ));
    }
    Ok(inspection)
}

fn native_codex_cli_unavailable(error: &str) -> bool {
    error.contains("failed to run codex") && error.contains("No such file or directory")
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
    let stdout = match run_codex_plugin_command(
        &["plugin".to_owned(), "list".to_owned(), "--json".to_owned()],
        project_root,
        Some(codex_home),
    ) {
        Ok(stdout) => stdout,
        Err(error) if native_codex_cli_unavailable(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
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

fn write_file_atomically(source_path: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let source_mode = fs::metadata(source_path)
                .map_err(|error| format!("failed to stat {}: {error}", source_path.display()))?
                .permissions()
                .mode();
            file.set_permissions(fs::Permissions::from_mode(source_mode))
                .map_err(|error| {
                    format!(
                        "failed to preserve mode on {}: {error}",
                        temporary.display()
                    )
                })?;
        }
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
