#[cfg(unix)]
mod unix {
    use super::materialize_plugin_install_state;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn unavailable_runtime_returns_a_successful_codex_deny_envelope() {
        let root = temp_project_root("codex-hook-unavailable-fail-closed");
        let state_home = root.join(".state");
        std::fs::create_dir_all(&state_home).expect("create isolated ASP state home");
        let mut child = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("ASP_STATE_HOME", &state_home)
            .args(["hook", "pre-tool", "--client", "codex"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn isolated unavailable Hook");
        serde_json::to_writer(
            child.stdin.as_mut().expect("Hook stdin"),
            &serde_json::json!({
                "session_id": "unavailable-runtime-regression",
                "cwd": root.display().to_string(),
                "hook_event_name": "PreToolUse",
                "tool_name": "Read",
                "tool_input": { "file_path": "src/lib.rs" }
            }),
        )
        .expect("write Hook payload");
        child
            .stdin
            .as_mut()
            .expect("Hook stdin")
            .flush()
            .expect("flush Hook payload");
        drop(child.stdin.take());
        let output = child.wait_with_output().expect("wait for unavailable Hook");
        assert!(
            output.status.success(),
            "unavailable enforcement must use the host deny protocol, stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("parse unavailable Hook response");
        assert_eq!(
            response["hookSpecificOutput"]["hookEventName"],
            "PreToolUse"
        );
        assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            response["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|context| context
                    .contains("agent.semantic-protocols.hook-local-policy-unavailable")),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_runs_project_installer() {
        let root = temp_project_root("codex-plugin-unified-install");
        let codex_home = root.join(".codex-home");
        let state_home = root.join(".state");
        let agent_bin_dir = root.join(".agent-bin");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&agent_bin_dir).expect("create semantic agent bin dir");
        materialize_plugin_install_state(&root, &state_home);
        std::fs::write(
            codex_home.join("config.toml"),
            "# BEGIN agent-semantic-protocol agent hooks\n[[hooks.pre_tool_use]]\nmatcher = \"*\"\n[[hooks.pre_tool_use.hooks]]\ntype = \"command\"\ncommand = \"direnv exec . asp-codex-hook pre-tool\"\n# END agent-semantic-protocol agent hooks\n",
        )
        .expect("write legacy global hook config");
        let agent_config_path = root.join(".agents").join("asp.toml");
        std::fs::create_dir_all(agent_config_path.parent().expect("agent config parent"))
            .expect("create agent config parent");
        std::fs::write(&agent_config_path, "[providers.org]\nenabled = false\n")
            .expect("write canonical agent config");
        write_existing_project_plugin_cache(&root);
        write_tracked_plugin_source_bundle(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_paths(&[&agent_bin_dir, &fake_bin]))
            .env("SEMANTIC_AGENT_BIN_DIR", &agent_bin_dir)
            .env("ASP_STATE_HOME", &state_home)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(["install", "plugin", "--codex", "--project", "."])
            .output()
            .expect("run asp install plugin --codex");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(agent_bin_dir.join("asp").is_file());
        assert!(Path::new(env!("CARGO_BIN_EXE_asp")).is_file());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("[plugin-install]"), "stdout={stdout}");
        assert!(
            stdout.contains("userConfigStatus=created"),
            "stdout={stdout}"
        );
        assert!(stdout.contains("pluginScope=project"), "stdout={stdout}");
        assert!(
            stdout.contains("pluginMarketplace=asp-project"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("pluginPayloadStatus=validated"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("hookAuthority=codex-plugin"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("agentConfigSync=not-on-plugin-install"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("agentConfig=not-on-plugin-install"),
            "stdout={stdout}"
        );
        assert!(!project_plugin_cache_root(&root).exists());
        let project_config = std::fs::read_to_string(root.join(".codex").join("config.toml"))
            .expect("read Codex project config");
        assert!(project_config.contains("[features]"), "{project_config}");
        assert!(project_config.contains("hooks = true"), "{project_config}");
        assert!(
            project_config.contains("plugins = true"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("# BEGIN agent-semantic-protocol agent hooks"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("[[hooks.pre_tool_use]]"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("[[hooks.PreToolUse]]"),
            "{project_config}"
        );
        assert!(!project_config.contains("direnv exec"), "{project_config}");
        assert!(
            !project_config.contains(" hook pre-tool "),
            "{project_config}"
        );
        assert!(
            !project_config.contains("\"$repo_root/.bin/asp\" hook"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("[agents.asp_explorer]"),
            "{project_config}"
        );
        assert!(!codex_home.join("agents").join("asp_explorer.toml").exists());
        assert!(
            !codex_home.join("agents").join("asp-explorer.toml").exists(),
            "legacy Codex explorer alias must be retired"
        );
        assert!(
            !codex_home.join("agents").join("asp-testing.toml").exists(),
            "legacy Codex testing alias must be retired"
        );
        let global_config = std::fs::read_to_string(codex_home.join("config.toml"))
            .expect("read global Codex config");
        assert!(
            !global_config.contains("[[hooks.pre_tool_use]]"),
            "{global_config}"
        );
        assert!(!global_config.contains("direnv exec"), "{global_config}");
        assert!(
            !global_config.contains("\"$repo_root/.bin/asp\" hook"),
            "{global_config}"
        );
        assert!(
            !global_config.contains("nickname_candidates"),
            "{global_config}"
        );
        assert!(
            !global_config
                .contains("asp-codex-plugin@asp-project:hooks/hooks.json:pre_tool_use:0:0"),
            "{global_config}"
        );
        let agent_config = std::fs::read_to_string(root.join(".agents").join("asp.toml"))
            .expect("read agent config");
        assert!(agent_config.contains("[providers.org]"), "{agent_config}");
        assert!(agent_config.contains("enabled = false"), "{agent_config}");

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_agent_projection_reads_project_registry() {
        let root = temp_project_root("codex-plugin-subagent-model-from-config");
        let codex_home = root.join(".codex-home");
        let test_home = root.join(".home");
        let state_home = root.join(".state");
        let agent_bin_dir = root.join(".agent-bin");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&test_home).expect("create isolated test home");
        materialize_plugin_install_state(&root, &state_home);
        install_test_asp_launcher(&agent_bin_dir);
        std::fs::create_dir_all(root.join(".codex")).expect("create project codex dir");
        std::fs::write(
            root.join("agents").join("asp_explorer_codex.toml"),
            r#"name = "asp_explorer"
description = "ASP search/query evidence explorer."
nickname_candidates = ["ASP Explore", "ASP Reasoning", "ASP Search"]
model = "gpt-5.3-codex-spark"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "test projection"
"#,
        )
        .expect("write project-owned ASP Explorer projection");
        write_existing_project_plugin_cache(&root);
        write_tracked_plugin_source_bundle(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("HOME", &test_home)
            .env("PATH", prepend_paths(&[&agent_bin_dir, &fake_bin]))
            .env("ASP_STATE_HOME", &state_home)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(["install", "plugin", "--codex", "."])
            .output()
            .expect("run asp install plugin --codex");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("pluginScope=global"), "stdout={stdout}");
        assert!(
            stdout.contains("agentConfigSync=not-on-plugin-install"),
            "stdout={stdout}"
        );
        assert!(
            !state_home
                .join("agents")
                .join("asp_explorer_codex.toml")
                .exists()
        );
        assert!(!codex_home.join("agents").join("asp_explorer.toml").exists());

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_preserves_tracked_source_bundle() {
        let root = temp_project_root("codex-plugin-tracked-source-bundle");
        let codex_home = root.join(".codex-home");
        let state_home = root.join(".state");
        let agent_bin_dir = root.join(".agent-bin");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        materialize_plugin_install_state(&root, &state_home);
        install_test_asp_launcher(&agent_bin_dir);
        write_tracked_plugin_source_bundle(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_paths(&[&agent_bin_dir, &fake_bin]))
            .env("ASP_STATE_HOME", &state_home)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(["install", "plugin", "--codex", "--project", "."])
            .output()
            .expect("run asp install plugin --codex");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            root.join("asp-codex-plugin")
                .join(".codex-plugin")
                .join("plugin.json")
                .is_file(),
            "tracked source plugin manifest must be preserved"
        );
        assert!(
            root.join("asp-codex-plugin")
                .join("hooks")
                .join("hooks.json")
                .is_file(),
            "production plugin source must preserve its canonical Hook bundle"
        );
        assert!(!project_plugin_cache_root(&root).exists());

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_help_uses_standard_sections_and_global_default() {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args(["install", "plugin", "--codex", "--help"])
            .output()
            .expect("run asp install plugin --codex --help");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage: asp install plugin"),
            "stdout={stdout}"
        );
        assert!(stdout.contains("Arguments:"), "stdout={stdout}");
        assert!(stdout.contains("Options:"), "stdout={stdout}");
        assert!(stdout.contains("globally"), "stdout={stdout}");
        assert!(
            stdout.contains("ASP_STATE_HOME [dev].root"),
            "stdout={stdout}"
        );
        assert!(!stdout.contains("--global-plugin"), "stdout={stdout}");
        assert!(!stdout.contains("--project-plugin"), "stdout={stdout}");
        assert!(!stdout.contains("[default: .]"), "stdout={stdout}");
    }

    #[test]
    fn install_plugin_codex_defaults_to_global_and_skips_project_plugin_cache() {
        let root = temp_project_root("codex-plugin-default-global-scope");
        let codex_home = root.join(".codex-home");
        let test_home = root.join(".home");
        let state_home = root.join(".state");
        let agent_bin_dir = root.join(".agent-bin");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(&test_home).expect("create isolated test home");
        materialize_plugin_install_state(&root, &state_home);
        install_test_asp_launcher(&agent_bin_dir);
        write_tracked_plugin_source_bundle(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("HOME", &test_home)
            .env("PATH", prepend_paths(&[&agent_bin_dir, &fake_bin]))
            .env("ASP_STATE_HOME", &state_home)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(["install", "plugin", "--codex", "."])
            .output()
            .expect("run asp install plugin --codex with default scope");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("pluginScope=global"), "stdout={stdout}");
        assert!(
            stdout.contains("pluginInstallStatus=updated"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("agentConfig=not-on-plugin-install"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("pluginInstalledPath=/tmp/asp-codex-plugin"),
            "stdout={stdout}"
        );
        let project_plugin_cache = root
            .join(".codex")
            .join("plugins")
            .join("cache")
            .join("asp-project");
        assert!(
            !project_plugin_cache.exists(),
            "global plugin install must not create project plugin cache: {}",
            project_plugin_cache.display()
        );

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    fn write_existing_project_plugin_cache(root: &Path) {
        let cache_skill_path = project_plugin_cache_root(root)
            .join("skills")
            .join("agent-semantic-protocols")
            .join("SKILL.org");
        std::fs::create_dir_all(cache_skill_path.parent().expect("cache skill dir"))
            .expect("create cache skill dir");
        std::fs::write(
            &cache_skill_path,
            "* ASP\nexisting project plugin cache skill\n",
        )
        .expect("write existing plugin cache skill");
    }

    fn write_tracked_plugin_source_bundle(root: &Path) {
        let plugin_root = root.join("asp-codex-plugin");
        let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
        std::fs::create_dir_all(manifest_path.parent().expect("plugin manifest dir"))
            .expect("create plugin manifest dir");
        std::fs::write(
            &manifest_path,
            include_bytes!("../../../../asp-codex-plugin/.codex-plugin/plugin.json"),
        )
        .expect("write canonical plugin manifest");
        let hooks_path = plugin_root.join("hooks").join("hooks.json");
        std::fs::create_dir_all(hooks_path.parent().expect("plugin hooks dir"))
            .expect("create plugin hooks dir");
        std::fs::write(
            &hooks_path,
            include_bytes!("../../../../asp-codex-plugin/hooks/hooks.json"),
        )
        .expect("write canonical plugin hooks");
        run_git(root, &["init"]);
        run_git(root, &["add", "asp-codex-plugin"]);
    }

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn project_plugin_cache_root(root: &Path) -> PathBuf {
        root.join(".codex")
            .join("plugins")
            .join("cache")
            .join("asp-project")
            .join("asp-codex-plugin")
            .join("0.1.0")
    }

    pub(super) fn temp_project_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-semantic-hook-{name}-{unique}"));
        std::fs::create_dir_all(&root).expect("create temp project root");
        std::fs::create_dir_all(root.join(".git")).expect("create git marker");
        root
    }

    fn write_fake_codex_cli(root: &Path) -> PathBuf {
        let bin_dir = root.join(".fake-bin");
        std::fs::create_dir_all(&bin_dir).expect("create fake bin dir");
        let codex = bin_dir.join("codex");
        std::fs::write(
            &codex,
            r#"#!/bin/sh
case "$*" in
  "plugin marketplace add "*)
    printf '{}\n'
    ;;
  "plugin marketplace list --json")
    printf '{"marketplaces":[]}\n'
    ;;
  "plugin add "*)
    printf '{"installedPath":"/tmp/asp-codex-plugin"}\n'
    ;;
  "plugin list --json")
    printf '{"installed":[{"pluginId":"asp-codex-plugin@asp-project","enabled":true}]}\n'
    ;;
  *)
    echo "unexpected codex command: $*" >&2
    exit 1
    ;;
esac
"#,
        )
        .expect("write fake codex cli");
        let mut permissions = std::fs::metadata(&codex)
            .expect("fake codex metadata")
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&codex, permissions).expect("chmod fake codex cli");
        bin_dir
    }

    fn install_test_asp_launcher(agent_bin_dir: &Path) {
        std::fs::create_dir_all(agent_bin_dir).expect("create test agent bin dir");
        std::fs::copy(env!("CARGO_BIN_EXE_asp"), agent_bin_dir.join("asp"))
            .expect("copy test asp launcher");
    }

    fn prepend_paths(bin_dirs: &[&Path]) -> String {
        let mut paths = std::env::split_paths(std::ffi::OsStr::new(
            "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        ))
        .collect::<Vec<_>>();
        for bin_dir in bin_dirs.iter().rev() {
            paths.insert(0, (*bin_dir).to_path_buf());
        }
        std::env::join_paths(paths)
            .expect("join PATH")
            .to_string_lossy()
            .into_owned()
    }
}
#[test]
fn claude_install_creates_managed_hook_config_and_sidecar() {
    let root = unix::temp_project_root("managed-hook-config-create");
    let output = run_claude_hook_install(&root);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=created"));
    let config = managed_hook_config_path(&root);
    assert!(config.is_file());
    assert!(managed_config_sidecar(&config).is_file());
    assert_no_managed_config_temporaries(config.parent().expect("config parent"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn claude_install_rejects_unproven_custom_config_without_stamping() {
    let root = unix::temp_project_root("managed-hook-config-custom");
    let config = managed_hook_config_path(&root);
    let mut custom =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse template");
    custom
        .as_table_mut()
        .expect("template table")
        .remove("contractFingerprint");
    custom["rules"][0]["message"] = toml::Value::String("custom managed rule message".to_string());
    std::fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    let custom_bytes = toml::to_string(&custom).expect("render custom");
    std::fs::write(&config, &custom_bytes).expect("write custom");
    let output = run_claude_hook_install(&root);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("user-config-contract-unproven"));
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        custom_bytes
    );
    assert!(!managed_config_sidecar(&config).exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn claude_install_rejects_unproven_config_without_fingerprint() {
    let root = unix::temp_project_root("managed-hook-config-unproven");
    let config = managed_hook_config_path(&root);
    let mut unproven =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse template");
    unproven
        .as_table_mut()
        .expect("template table")
        .remove("contractFingerprint");
    std::fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    let unproven_bytes = toml::to_string(&unproven).expect("render unproven config");
    std::fs::write(&config, &unproven_bytes).expect("write unproven config");

    let output = run_claude_hook_install(&root);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("user-config-contract-unproven"));
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        unproven_bytes
    );
    assert!(!managed_config_sidecar(&config).exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn claude_install_migrates_stale_managed_config_when_sidecar_proves_ownership() {
    let root = unix::temp_project_root("managed-hook-config-stale-sidecar");
    let config = managed_hook_config_path(&root);
    let mut stale =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse template");
    stale["contractFingerprint"] = toml::Value::String("stale-contract".to_string());
    let stale_bytes = toml::to_string(&stale).expect("render stale");
    std::fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    std::fs::write(&config, &stale_bytes).expect("write stale");
    std::fs::write(
        managed_config_sidecar(&config),
        test_sha256(stale_bytes.as_bytes()),
    )
    .expect("write sidecar");
    let output = run_claude_hook_install(&root);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("userConfigStatus=migrated-managed"));
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        agent_semantic_hook::default_client_config_template()
    );
    assert_no_managed_config_temporaries(config.parent().expect("config parent"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn run_claude_hook_install(root: &std::path::Path) -> std::process::Output {
    let state_home = root.join(".state");
    materialize_plugin_install_state(root, &state_home);
    let asp_bin_dir = root.join(".agent-bin");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
    paths.insert(0, asp_bin_dir.clone());
    let path = std::env::join_paths(paths).expect("join PATH");
    std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .env("PATH", path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", state_home)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "hook", "--client", "claude", "."])
        .output()
        .expect("run claude hook install")
}

fn materialize_plugin_install_state(root: &std::path::Path, state_home: &std::path::Path) {
    crate::unit_state_home_fixture::materialize_org_state_checkout(state_home);
    crate::state_home_fixture::install_provider_script(state_home, "rust", "#!/bin/sh\nexit 0\n");
    crate::state_home_fixture::write_activation(root, state_home, &["rust"]);
    let project_agents = root.join("agents");
    std::fs::create_dir_all(&project_agents).expect("create project agents directory");
    for (name, bytes) in [
        (
            "config.toml",
            include_bytes!("../../../../agents/config.toml").as_slice(),
        ),
        (
            "asp_explorer_codex.toml",
            include_bytes!("../../../../agents/asp_explorer_codex.toml").as_slice(),
        ),
        (
            "asp_explorer_claude.md",
            include_bytes!("../../../../agents/asp_explorer_claude.md").as_slice(),
        ),
        (
            "asp_testing_codex.toml",
            include_bytes!("../../../../agents/asp_testing_codex.toml").as_slice(),
        ),
        (
            "asp_testing_claude.md",
            include_bytes!("../../../../agents/asp_testing_claude.md").as_slice(),
        ),
    ] {
        std::fs::write(project_agents.join(name), bytes).expect("write project agent registry");
    }
}

fn managed_config_sidecar(config: &std::path::Path) -> std::path::PathBuf {
    config.with_file_name(format!(
        "{}.managed.sha256",
        config
            .file_name()
            .and_then(|name| name.to_str())
            .expect("config name")
    ))
}

fn managed_hook_config_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".state").join("hooks").join("config.toml")
}

fn test_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn assert_no_managed_config_temporaries(dir: &std::path::Path) {
    let temporaries = std::fs::read_dir(dir)
        .expect("read config directory")
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(
        temporaries,
        0,
        "managed config temporaries remain in {}",
        dir.display()
    );
}
