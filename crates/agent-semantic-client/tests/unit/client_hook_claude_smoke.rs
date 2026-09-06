// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::Value;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct InstallFixture {
    root: PathBuf,
}

impl Drop for InstallFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn install_fixture() -> InstallFixture {
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "asp-client-hook-install-smoke-{}-{}-{unique}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
    ));
    std::fs::create_dir_all(&root).expect("create install fixture root");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("initialize fixture repository");
    let org_fixture = root.join(".agent-semantic-protocols/org");
    std::fs::create_dir_all(&org_fixture).expect("create fixture Org state");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&org_fixture)
        .status()
        .expect("initialize fixture Org repository");
    Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/tao3k/org.git",
        ])
        .current_dir(&org_fixture)
        .status()
        .expect("configure fixture Org origin");
    std::fs::write(org_fixture.join(".fixture"), "local-only\n")
        .expect("mark fixture Org checkout dirty");
    std::fs::create_dir_all(root.join("src")).expect("create fixture source root");
    std::fs::write(root.join("src/lib.rs"), "pub fn demo() {}\n").expect("write fixture source");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create fixture bin root");
    std::fs::hard_link(env!("CARGO_BIN_EXE_asp"), bin_dir.join("asp"))
        .expect("link local regular ASP test artifact");
    let state_home = crate::unit_state_home_fixture::default_state_home(&root);
    crate::state_home_fixture::install_provider_script(&state_home, "rust", "#!/bin/sh\nexit 0\n");
    crate::state_home_fixture::write_activation(&root, &state_home, &["rust"]);
    write_agent_config_fixture(&root);
    write_test_codex_plugin(&root);
    write_fake_codex_cli(&bin_dir);
    InstallFixture { root }
}

struct InstallEnvironment {
    home: PathBuf,
    codex_home: PathBuf,
    state_home: PathBuf,
    bin_dir: PathBuf,
}

impl InstallEnvironment {
    fn new(root: &Path) -> Self {
        let home = root.join(".home");
        let codex_home = root.join(".codex-home");
        let state_home = root.join(".agent-semantic-protocols");
        let bin_dir = root.join(".bin");
        for path in [&home, &codex_home, &state_home, &bin_dir] {
            std::fs::create_dir_all(path).expect("create install environment root");
        }
        Self {
            home,
            codex_home,
            state_home,
            bin_dir,
        }
    }

    fn apply(&self, command: &mut Command) {
        command.env_clear();
        command
            .env("PATH", prepend_path(&self.bin_dir))
            .env("HOME", &self.home)
            .env("CODEX_HOME", &self.codex_home)
            .env("ASP_STATE_HOME", &self.state_home)
            .env("SEMANTIC_AGENT_BIN_DIR", &self.bin_dir)
            .env("TMPDIR", self.home.join("tmp"));
    }
}

#[test]
fn claude_install_writes_shared_tool_surface_hooks() {
    let fixture = install_fixture();
    let environment = InstallEnvironment::new(&fixture.root);
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["install", "hook", "--client", "claude"])
        .arg(&fixture.root);
    environment.apply(&mut command);
    let output = command.output().expect("install Claude hooks");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let settings: Value = serde_json::from_slice(
        &std::fs::read(fixture.root.join(".claude/settings.json")).expect("read Claude settings"),
    )
    .expect("parse Claude settings");
    let matcher = settings["hooks"]["PreToolUse"][0]["matcher"]
        .as_str()
        .expect("Claude PreToolUse matcher");
    assert_ne!(matcher, "*");
    assert!(matcher.contains("functions\\.exec|"));
    assert!(matcher.contains("functions\\.exec_command"));
    assert_eq!(settings["hooks"]["PostToolUse"][0]["matcher"], matcher);
    assert!(settings["hooks"].get("PermissionRequest").is_none());
}

#[test]
fn codex_project_install_writes_plugin_and_configured_agent_projection() {
    let fixture = install_fixture();
    let environment = InstallEnvironment::new(&fixture.root);
    let output = run_codex_project_install(&fixture.root, &environment);
    assert!(
        output.status.success(),
        "install stdout: {}\ninstall stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("install stdout is UTF-8");
    assert!(stdout.contains("pluginScope=project"), "{stdout}");
    assert!(stdout.contains("activationSync=refreshed"), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "binaryPath={}",
            fixture.root.join(".bin/asp").display()
        )),
        "{stdout}"
    );
    let receipt_field = stdout
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix("activeArtifactReceipt="))
        .expect("project install active artifact receipt");
    let receipt_path = PathBuf::from(receipt_field);
    let receipt_path = if receipt_path.is_absolute() {
        receipt_path
    } else {
        fixture.root.join(receipt_path)
    };
    assert!(
        receipt_path.starts_with(fixture.root.join(".agent-semantic-protocols")),
        "receipt escaped fixture State Home: {}",
        receipt_path.display()
    );
    let project_config = std::fs::read_to_string(fixture.root.join(".codex/config.toml"))
        .expect("read project Codex config");
    assert!(project_config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert!(!project_config.contains("[agents.asp_explorer]"));
    let agent = std::fs::read_to_string(environment.codex_home.join("agents/asp_explorer.toml"))
        .expect("read configured agent projection");
    assert!(agent.contains("name = \"asp_explorer\""));
    assert!(agent.contains("narrowest parser-owned ASP route"));
    let cache_field = stdout
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix("pluginCache="))
        .expect("project install receipt pluginCache");
    let cache_path = PathBuf::from(cache_field);
    let cache_path = if cache_path.is_absolute() {
        cache_path
    } else {
        fixture.root.join(cache_path)
    };
    let skill_path = cache_path.join("skills/agent-semantic-protocols/SKILL.org");
    assert!(
        skill_path.is_file(),
        "plugin skill: {}",
        skill_path.display()
    );
}

#[test]
fn codex_project_install_rejects_unrelated_path_asp_without_mutation() {
    let fixture = install_fixture();
    let environment = InstallEnvironment::new(&fixture.root);
    let ambient_bin = fixture.root.join(".ambient-bin");
    std::fs::create_dir_all(&ambient_bin).expect("create ambient bin");
    let ambient_asp = ambient_bin.join("asp");
    std::fs::write(&ambient_asp, b"ambient-sentinel").expect("write ambient ASP sentinel");
    make_executable(&ambient_asp);
    let before = std::fs::read(&ambient_asp).expect("snapshot ambient ASP sentinel");
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["install", "plugin", "--codex", "--project"])
        .arg(&fixture.root);
    environment.apply(&mut command);
    command
        .env_remove("SEMANTIC_AGENT_BIN_DIR")
        .env("PATH", prepend_path(&ambient_bin));
    let output = command.output().expect("run rejected Codex install");
    assert!(!output.status.success(), "install unexpectedly succeeded");
    assert!(String::from_utf8_lossy(&output.stderr).contains("Codex App PATH resolves stale ASP"));
    assert_eq!(
        std::fs::read(&ambient_asp).expect("read ambient ASP sentinel"),
        before
    );
}

fn run_codex_project_install(
    root: &Path,
    environment: &InstallEnvironment,
) -> std::process::Output {
    let hook_config = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(hook_config.parent().expect("hook config parent"))
        .expect("create hook config root");
    std::fs::write(
        &hook_config,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("write hook config");
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["install", "plugin", "--codex", "--project"])
        .arg(root);
    environment.apply(&mut command);
    command.output().expect("run Codex project install")
}

fn write_agent_config_fixture(root: &Path) {
    let agents = root.join("agents");
    std::fs::create_dir_all(&agents).expect("create agent config fixture");
    std::fs::write(
        agents.join("config.toml"),
        r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[platforms.codex]
matcher = "*_codex.toml"

[agents.asp_explorer]
session_lifetime = "resident"
roles = ["explore", "subagent"]

[agents.asp_testing]
session_lifetime = "resident"
roles = ["build", "subagent", "testing"]

"#,
    )
    .expect("write agent route registry fixture");
    for (file, name, sandbox_mode, instructions) in [
        (
            "asp_explorer_codex.toml",
            "asp_explorer",
            "read-only",
            "Execute the narrowest parser-owned ASP route and return one asp.search.playbook-receipt.",
        ),
        (
            "asp_testing_codex.toml",
            "asp_testing",
            "workspace-write",
            "Run the routed build or test command exactly once.",
        ),
    ] {
        std::fs::write(
            agents.join(file),
            format!(
                "name = \"{name}\"\ndescription = \"ASP test fixture.\"\nmodel = \"gpt-5.6-luna\"\nmodel_reasoning_effort = \"low\"\nsandbox_mode = \"{sandbox_mode}\"\ndeveloper_instructions = \"{instructions}\"\n"
            ),
        )
        .expect("write Codex agent projection fixture");
    }
}

fn write_test_codex_plugin(root: &Path) {
    let plugin_root = root.join("asp-codex-plugin");
    let manifest = plugin_root.join(".codex-plugin/plugin.json");
    std::fs::create_dir_all(manifest.parent().expect("plugin manifest parent"))
        .expect("create plugin manifest root");
    std::fs::write(
        manifest,
        r#"{"name":"asp-codex-plugin","version":"0.1.0+test","description":"Test ASP Codex plugin","author":{"name":"ASP"},"skills":"./skills/","interface":{"displayName":"ASP Test"}}"#,
    )
    .expect("write plugin manifest");
    let hook_config = plugin_root.join("templates/hooks/config.toml");
    std::fs::create_dir_all(hook_config.parent().expect("plugin hook config parent"))
        .expect("create plugin hook config root");
    std::fs::write(
        hook_config,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("write plugin hook config template");
}

fn write_fake_codex_cli(bin_dir: &Path) {
    let path = bin_dir.join("codex");
    std::fs::write(
        &path,
        r#"#!/bin/sh
set -eu
codex_home="${CODEX_HOME:-${HOME:-}/.codex}"
config="$codex_home/config.toml"
/bin/mkdir -p "${config%/*}"
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "add" ]; then
  root="${4:-.}"
  { printf '[marketplaces.asp-project]\n'; printf 'last_updated = "2026-01-01T00:00:00Z"\n'; printf 'source_type = "local"\n'; printf 'source = "%s"\n\n' "$root"; } >> "$config"
  printf '{"marketplaceName":"asp-project","installedRoot":"%s","alreadyAdded":false}\n' "$root"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "add" ]; then
  /bin/mkdir -p "$codex_home/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test"
  { printf '[plugins."asp-codex-plugin@asp-project"]\n'; printf 'enabled = true\n'; } >> "$config"
  printf '{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installedPath":"%s/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test","authPolicy":"ON_INSTALL"}\n' "$codex_home"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "list" ]; then
  printf '{"installed":[{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installed":true,"enabled":true}],"available":[]}\n'
  exit 0
fi
printf 'unsupported fake codex command: %s\n' "$*" >&2
exit 2
"#,
    )
    .expect("write fake Codex CLI");
    make_executable(&path);
}

fn prepend_path(prefix: &Path) -> OsString {
    let mut paths = vec![prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("file metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("file permissions");
    }
    #[cfg(not(unix))]
    let _ = path;
}
