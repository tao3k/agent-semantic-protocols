use crate::rust_harness_activation::support::{asp_command, temp_project_root};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn git_project_root(name: &str) -> PathBuf {
    let root = temp_project_root(name);
    std::fs::create_dir_all(root.join(".git")).expect("create temp git toplevel marker");
    write_test_codex_plugin(&root);
    write_fake_codex_cli(&root);
    root
}

pub(super) fn protocol_command() -> Command {
    let mut command = asp_command();
    command.env_remove("PRJ_CACHE_HOME");
    command
}

pub(super) fn codex_plugin_install_args(root: &Path) -> [String; 4] {
    [
        "plugin".to_string(),
        "install".to_string(),
        "codex".to_string(),
        root.to_str().expect("utf8 temp root").to_string(),
    ]
}

pub(super) fn codex_plugin_install_args_with_subagent_model(
    root: &Path,
    model: &str,
) -> [String; 6] {
    [
        "plugin".to_string(),
        "install".to_string(),
        "codex".to_string(),
        "--subagent-model".to_string(),
        model.to_string(),
        root.to_str().expect("utf8 temp root").to_string(),
    ]
}

fn write_test_codex_plugin(root: &Path) {
    let plugin_root = root.join("asp-codex-plugin");
    let manifest = plugin_root.join(".codex-plugin/plugin.json");
    std::fs::create_dir_all(manifest.parent().expect("plugin manifest parent"))
        .expect("create plugin manifest dir");
    std::fs::write(
        &manifest,
        r#"{
  "name": "asp-codex-plugin",
  "version": "0.1.0+test",
  "description": "Test ASP Codex plugin",
  "author": {"name": "ASP"},
  "skills": "./skills/",
  "hooks": "./hooks/hooks.json",
  "interface": {"displayName": "ASP Test"}
}
"#,
    )
    .expect("write plugin manifest");
    let hooks = plugin_root.join("hooks/hooks.json");
    std::fs::create_dir_all(hooks.parent().expect("plugin hooks parent"))
        .expect("create plugin hooks dir");
    std::fs::write(&hooks, r#"{"hooks":{}}"#).expect("write plugin hooks");
}

fn write_fake_codex_cli(root: &Path) {
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fake Codex bin dir");
    let path = bin_dir.join("codex");
    std::fs::write(
        &path,
        r#"#!/bin/sh
set -eu
codex_home="${CODEX_HOME:-${HOME:-}/.codex}"
config="$codex_home/config.toml"
config_dir="${config%/*}"
/bin/mkdir -p "$config_dir"
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "add" ]; then
  root="${4:-.}"
  if /usr/bin/grep -q '^\[marketplaces\.asp-project\]' "$config" 2>/dev/null; then
    existing_source="$(/usr/bin/awk -F'"' '/^source = / {print $2; exit}' "$config")"
    if [ "$existing_source" != "$root" ]; then
      printf "Error: marketplace 'asp-project' is already added from a different source; remove it before adding this source\n" >&2
      exit 1
    fi
    printf '{"marketplaceName":"asp-project","installedRoot":"%s","alreadyAdded":true}\n' "$root"
    exit 0
  else
    {
      printf '[marketplaces.asp-project]\n'
      printf 'last_updated = "2026-01-01T00:00:00Z"\n'
      printf 'source_type = "local"\n'
      printf 'source = "%s"\n\n' "$root"
    } >> "$config"
  fi
  printf '{"marketplaceName":"asp-project","installedRoot":"%s","alreadyAdded":false}\n' "$root"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "list" ]; then
  if /usr/bin/grep -q '^\[marketplaces\.asp-project\]' "$config" 2>/dev/null; then
    source="$(/usr/bin/awk -F'"' '/^source = / {print $2; exit}' "$config")"
    case "$source" in
      /*) root="$source" ;;
      *) root="$(cd "$source" && pwd -P)" ;;
    esac
    printf '{"marketplaces":[{"name":"asp-project","root":"%s"}]}\n' "$root"
  else
    printf '{"marketplaces":[]}\n'
  fi
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "add" ]; then
  /bin/mkdir -p "$codex_home/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test"
  if /usr/bin/grep -q '^\[marketplaces\.asp-project\]' "$config" 2>/dev/null; then
    /usr/bin/awk '
      /^\[marketplaces\.asp-project\]/ { in_marketplace = 1; print; next }
      /^\[/ { in_marketplace = 0 }
      in_marketplace == 1 && /^source = / { print "source = \"..\""; next }
      { print }
    ' "$config" > "$config.tmp"
    /bin/mv "$config.tmp" "$config"
  fi
  if /usr/bin/grep -q '^\[agents\.asp_explorer\]' "$config" 2>/dev/null; then
    /usr/bin/awk '
      /^\[agents\.asp_explorer\]/ { skip = 1; next }
      /^\[/ { skip = 0 }
      skip != 1 { print }
    ' "$config" > "$config.tmp"
    /bin/mv "$config.tmp" "$config"
  fi
  if ! /usr/bin/grep -q '^\[plugins\."asp-codex-plugin@asp-project"\]' "$config" 2>/dev/null; then
    {
      printf '[plugins."asp-codex-plugin@asp-project"]\n'
      printf 'enabled = true\n'
    } >> "$config"
  fi
  printf '{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installedPath":"%s/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test","authPolicy":"ON_INSTALL"}\n' "$codex_home"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "list" ]; then
  if /usr/bin/grep -q '^\[plugins\."asp-codex-plugin@asp-project"\]' "$config" 2>/dev/null; then
    printf '{"installed":[{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installed":true,"enabled":true}],"available":[]}\n'
  else
    printf '{"installed":[],"available":[]}\n'
  fi
  exit 0
fi
printf 'unsupported fake codex command: %s\n' "$*" >&2
exit 2
"#,
    )
    .expect("write fake Codex CLI");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path)
            .expect("fake Codex metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fake Codex CLI");
    }
}
