// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Codex plugin payload identity and installed-cache comparison.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Relative path of the canonical Codex plugin manifest.
pub const CODEX_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".codex-plugin/plugin.json";
/// Relative path of the canonical Codex Hook routing payload.
pub const CODEX_PLUGIN_HOOKS_RELATIVE_PATH: &str = "hooks/hooks.json";
/// Relative path of the fixed Codex Hook launcher.
pub const CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH: &str = "bin/asp-hook-exec";

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"agent.semantic-protocols.codex-plugin-payload\0";

/// Content identity of the three-file Codex plugin payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexPluginPayloadIdentity {
    /// Plugin name declared by the manifest.
    pub plugin_name: String,
    /// Cache-busting plugin version declared by the manifest.
    pub version: String,
    /// Domain-separated digest of manifest, Hook routing, and launcher bytes.
    pub digest: String,
}

/// Relationship between a validated source payload and the global installed cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexPluginPayloadState {
    Current,
    PublicationRequired,
    Missing,
    Corrupt,
}

impl CodexPluginPayloadState {
    /// Stable receipt value for this payload state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::PublicationRequired => "plugin-payload-publication-required",
            Self::Missing => "plugin-cache-missing",
            Self::Corrupt => "installed-cache-corrupt",
        }
    }
}

/// Typed result of comparing source payload authority with one installed cache root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexPluginPayloadInspection {
    /// Classified source-to-cache relationship.
    pub state: CodexPluginPayloadState,
    /// Validated identity of the source payload.
    pub source: CodexPluginPayloadIdentity,
    /// Validated installed identity when the cache is readable.
    pub installed: Option<CodexPluginPayloadIdentity>,
    /// Global Codex cache root inspected for this plugin version.
    pub installed_root: PathBuf,
    /// Typed diagnostic detail for missing or corrupt cache state.
    pub detail: Option<String>,
}

/// Build the deterministic global Codex cache path for a plugin version.
pub fn codex_plugin_cache_root(
    codex_home: &Path,
    marketplace_name: &str,
    plugin_name: &str,
    version: &str,
) -> PathBuf {
    codex_home
        .join("plugins")
        .join("cache")
        .join(marketplace_name)
        .join(plugin_name)
        .join(version)
}

/// Compare one validated source payload with a concrete installed cache root.
pub fn inspect_codex_plugin_payload(
    source_root: &Path,
    installed_root: &Path,
) -> Result<CodexPluginPayloadInspection, String> {
    let source = load_codex_plugin_payload_identity(source_root)
        .map_err(|error| format!("invalid source Codex plugin payload: {error}"))?;
    if !installed_root.is_dir() {
        return Ok(CodexPluginPayloadInspection {
            state: CodexPluginPayloadState::Missing,
            source,
            installed: None,
            installed_root: installed_root.to_path_buf(),
            detail: Some("installed plugin cache root does not exist".to_owned()),
        });
    }
    let installed = match load_codex_plugin_payload_identity(installed_root) {
        Ok(installed) => installed,
        Err(error) => {
            return Ok(CodexPluginPayloadInspection {
                state: CodexPluginPayloadState::Corrupt,
                source,
                installed: None,
                installed_root: installed_root.to_path_buf(),
                detail: Some(error),
            });
        }
    };
    let state = if source == installed {
        CodexPluginPayloadState::Current
    } else {
        CodexPluginPayloadState::PublicationRequired
    };
    Ok(CodexPluginPayloadInspection {
        state,
        source,
        installed: Some(installed),
        installed_root: installed_root.to_path_buf(),
        detail: None,
    })
}

/// Validate and digest a Codex plugin payload rooted at `plugin_root`.
pub fn load_codex_plugin_payload_identity(
    plugin_root: &Path,
) -> Result<CodexPluginPayloadIdentity, String> {
    let payload = read_plugin_payload(plugin_root)?;
    let (plugin_name, version) = decode_manifest_identity(plugin_root, &payload.manifest)?;
    validate_hooks_payload(plugin_root, &payload.hooks)?;
    validate_launcher_payload(plugin_root, &payload.launcher)?;
    Ok(CodexPluginPayloadIdentity {
        plugin_name,
        version,
        digest: digest_plugin_payload(&payload),
    })
}

struct CodexPluginPayloadFiles {
    manifest: Vec<u8>,
    hooks: Vec<u8>,
    launcher: Vec<u8>,
}

fn read_plugin_payload(plugin_root: &Path) -> Result<CodexPluginPayloadFiles, String> {
    Ok(CodexPluginPayloadFiles {
        manifest: read_payload_file(plugin_root, CODEX_PLUGIN_MANIFEST_RELATIVE_PATH)?,
        hooks: read_payload_file(plugin_root, CODEX_PLUGIN_HOOKS_RELATIVE_PATH)?,
        launcher: read_payload_file(plugin_root, CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH)?,
    })
}

fn decode_manifest_identity(
    plugin_root: &Path,
    manifest_bytes: &[u8],
) -> Result<(String, String), String> {
    let manifest =
        serde_json::from_slice::<serde_json::Value>(manifest_bytes).map_err(|error| {
            format!(
                "invalid {}: {error}",
                plugin_root
                    .join(CODEX_PLUGIN_MANIFEST_RELATIVE_PATH)
                    .display()
            )
        })?;
    let plugin_name = required_manifest_string(&manifest, "name")?.to_owned();
    let version = required_manifest_string(&manifest, "version")?.to_owned();
    if manifest.get("hooks").is_some() || manifest.get("skills").is_some() {
        return Err(format!(
            "{} must remain a Hook-only standard-directory manifest without `hooks` or `skills` fields",
            plugin_root
                .join(CODEX_PLUGIN_MANIFEST_RELATIVE_PATH)
                .display()
        ));
    }
    Ok((plugin_name, version))
}

fn validate_hooks_payload(plugin_root: &Path, hooks: &[u8]) -> Result<(), String> {
    let hooks_value = serde_json::from_slice::<serde_json::Value>(hooks).map_err(|error| {
        format!(
            "invalid {}: {error}",
            plugin_root.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH).display()
        )
    })?;
    let events = hooks_value
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .filter(|events| !events.is_empty())
        .ok_or_else(|| {
            format!(
                "{} has no Hook event map",
                plugin_root.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH).display()
            )
        })?;
    for handler in events
        .values()
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(|group| group.get("hooks"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
    {
        if !handler
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|command| command.starts_with("\"$PLUGIN_ROOT/bin/asp-hook-exec\" "))
        {
            return Err(format!(
                "{} contains a Hook handler that bypasses the plugin-owned launcher",
                plugin_root.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH).display()
            ));
        }
    }
    Ok(())
}

fn validate_launcher_payload(plugin_root: &Path, launcher: &[u8]) -> Result<(), String> {
    let launcher_text = std::str::from_utf8(launcher).map_err(|error| {
        format!(
            "{} is not UTF-8: {error}",
            plugin_root
                .join(CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH)
                .display()
        )
    })?;
    if !launcher_text.starts_with("#!/bin/sh\n")
        || !launcher_text.contains("runtime/artifacts/active/asp-hook")
        || launcher_text.contains("hooks/current")
        || launcher_text.contains("ASP_HOOK_GENERATION_ROOT")
        || launcher_text.contains("runtime/bin/")
        || launcher_text.contains("runtime/artifacts/active")
    {
        return Err(format!(
            "{} must resolve only the canonical Runtime Hook binary",
            plugin_root
                .join(CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH)
                .display()
        ));
    }
    Ok(())
}

fn digest_plugin_payload(payload: &CodexPluginPayloadFiles) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    for (relative, bytes) in [
        (CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, &payload.manifest),
        (CODEX_PLUGIN_HOOKS_RELATIVE_PATH, &payload.hooks),
        (CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH, &payload.launcher),
    ] {
        hasher.update(relative.as_bytes());
        hasher.update(&[0]);
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn required_manifest_string<'a>(
    manifest: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, String> {
    manifest
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Codex plugin manifest is missing non-empty `{field}`"))
}

fn read_payload_file(plugin_root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = plugin_root.join(relative);
    fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "../tests/unit/codex_plugin_payload.rs"]
mod tests;
