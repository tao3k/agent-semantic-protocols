#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    const ASP_ORG_SKILL_TEMPLATE: &str =
        include_str!("../../../../languages/org/templates/ASP_ORG_SKILL.org");

    #[test]
    fn install_plugin_codex_runs_project_installer() {
        let root = temp_project_root("codex-plugin-unified-install");
        let codex_home = root.join(".codex-home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(root.join(".codex")).expect("create project codex dir");
        std::fs::write(
            root.join(".codex").join("config.toml"),
            "[marketplaces.asp-project]\nsource_type = \"local\"\nsource = \".\"\n",
        )
        .expect("write legacy project marketplace config");
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

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
            .env("SEMANTIC_AGENT_BIN_DIR", asp_bin_dir())
            .env("SEMANTIC_AGENT_BIN_DIR", asp_bin_dir())
            .env("SEMANTIC_AGENT_BIN_DIR", asp_bin_dir())
            .env("SEMANTIC_AGENT_BIN_DIR", asp_bin_dir())
            .env("ASP_STATE_HOME", root.join(".state"))
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("[plugin-install]"), "stdout={stdout}");
        assert!(
            stdout.contains("userConfigStatus=created"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains(
                "pluginSkill=.codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org"
            ),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("pluginCache=.codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains(
                "globalPluginCache=.codex-home/plugins/cache/asp-project/asp-codex-plugin/0.1.0"
            ),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("pluginSourceTrustConfig=.codex-home/config.toml"),
            "stdout={stdout}"
        );
        assert!(
            stdout.contains("pluginCacheTrustConfig=.codex-home/config.toml"),
            "stdout={stdout}"
        );
        assert_current_agent_config(&root);
        assert_project_plugin_cache_refreshed(&root);
        assert_global_plugin_cache_refreshed(&root, &codex_home);
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
        assert!(
            !project_config.contains("[marketplaces.asp-project]"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("source_type = \"local\""),
            "{project_config}"
        );
        assert!(
            !project_config.contains("source = \".\""),
            "{project_config}"
        );
        assert!(
            project_config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"),
            "{project_config}"
        );
        let explorer_agent =
            std::fs::read_to_string(codex_home.join("agents").join("asp-explorer.toml"))
                .expect("read Codex ASP Explorer agent");
        assert!(
            explorer_agent.contains(
                r#"nickname_candidates = ["ASP Explore", "ASP Reasoning", "ASP Search"]"#
            ),
            "{explorer_agent}"
        );
        assert!(
            explorer_agent.contains(r#"model_reasoning_effort = "low""#),
            "{explorer_agent}"
        );
        assert!(
            !explorer_agent.contains("session_lifetime"),
            "{explorer_agent}"
        );
        let global_config = std::fs::read_to_string(codex_home.join("config.toml"))
            .expect("read global Codex config");
        assert!(
            !global_config.contains("# BEGIN agent-semantic-protocol agent hooks"),
            "{global_config}"
        );
        assert!(
            !global_config.contains("[[hooks.pre_tool_use]]"),
            "{global_config}"
        );
        assert!(!global_config.contains("direnv exec"), "{global_config}");
        assert!(
            !global_config.contains(" hook pre-tool "),
            "{global_config}"
        );
        assert!(
            !global_config.contains(r#"repo_root="${CODEX_WORKSPACE_ROOT:-${PWD:-.}}""#),
            "{global_config}"
        );
        assert!(
            !global_config.contains("\"$repo_root/.bin/asp\" hook"),
            "{global_config}"
        );
        assert!(
            !global_config.contains("nickname_candidates"),
            "{global_config}"
        );
        assert!(
            !global_config.contains(".codex-home/config.toml:pre_tool_use:0:0"),
            "{global_config}"
        );
        assert!(
            global_config
                .contains("asp-codex-plugin@asp-project:hooks/hooks.json:pre_tool_use:0:0"),
            "{global_config}"
        );
        assert!(
            global_config.contains("agent-semantic-protocol trusted hook state"),
            "{global_config}"
        );
        let agent_config = std::fs::read_to_string(root.join(".agents").join("asp.toml"))
            .expect("read agent config");
        assert!(agent_config.contains("[providers.org]"), "{agent_config}");
        assert!(agent_config.contains("enabled = false"), "{agent_config}");

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_default_subagent_model_reads_asp_agents_config() {
        let root = temp_project_root("codex-plugin-subagent-model-from-config");
        let codex_home = root.join(".codex-home");
        let state_home = root.join(".state");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::create_dir_all(root.join(".codex")).expect("create project codex dir");
        std::fs::create_dir_all(state_home.join("agents")).expect("create ASP agents dir");
        std::fs::write(
            state_home.join("agents").join("config.toml"),
            r#"[platform.codex.models]
primary = "gpt-5.3-codex-spark"
fallback = ["gpt-5.4-mini"]
"#,
        )
        .expect("write ASP agents config");
        write_existing_project_plugin_cache(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
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

        let explorer_agent =
            std::fs::read_to_string(state_home.join("agents").join("asp-explorer_codex.toml"))
                .expect("read canonical ASP Explorer agent");
        assert!(
            explorer_agent.contains(r#"model = "gpt-5.3-codex-spark""#),
            "{explorer_agent}"
        );
        assert!(
            !explorer_agent.contains(r#"model = "gpt-5.4-mini""#),
            "{explorer_agent}"
        );

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_preserves_tracked_source_bundle() {
        let root = temp_project_root("codex-plugin-tracked-source-bundle");
        let codex_home = root.join(".codex-home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        write_tracked_plugin_source_bundle(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
            .env("ASP_STATE_HOME", root.join(".state"))
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
            "tracked source plugin hooks must be preserved"
        );
        let plugin_cache_root = root
            .join(".codex")
            .join("plugins")
            .join("cache")
            .join("asp-project")
            .join("asp-codex-plugin")
            .join("0.1.0");
        let plugin_manifest_path = plugin_cache_root.join(".codex-plugin").join("plugin.json");
        let plugin_manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&plugin_manifest_path).expect("read plugin manifest"),
        )
        .expect("parse plugin manifest");
        assert_eq!(
            plugin_manifest["hooks"].as_str(),
            Some("./hooks/hooks.json"),
            "Codex plugin hooks must use the native string path schema"
        );
        assert!(
            !plugin_manifest["hooks"].is_array(),
            "Codex plugin hooks must not be written as an array"
        );
        let hooks_path =
            plugin_cache_root.join(plugin_manifest["hooks"].as_str().expect("hooks path"));
        assert!(
            hooks_path.is_file(),
            "Codex plugin hooks path must resolve from plugin root: {}",
            hooks_path.display()
        );
        assert_project_plugin_cache_refreshed(&root);
        assert_global_plugin_cache_refreshed(&root, &codex_home);

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
        assert!(
            stdout.contains("Install globally (default when no scope flag is given)"),
            "stdout={stdout}"
        );
        assert!(stdout.contains("--project"), "stdout={stdout}");
        assert!(stdout.contains("[default: .]"), "stdout={stdout}");
    }

    #[test]
    fn install_plugin_codex_global_skips_project_plugin_cache() {
        let root = temp_project_root("codex-plugin-global-scope");
        let codex_home = root.join(".codex-home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
            .env("ASP_STATE_HOME", root.join(".state"))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args(["install", "plugin", "--codex", "--global", "."])
            .output()
            .expect("run asp install plugin --codex --global");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("pluginScope=global"), "stdout={stdout}");
        assert!(
            stdout.contains(
                "globalPluginCache=.codex-home/plugins/cache/asp-project/asp-codex-plugin/0.1.0"
            ),
            "stdout={stdout}"
        );
        assert!(
            !stdout.contains("pluginCache=.codex/plugins/cache/asp-project"),
            "stdout={stdout}"
        );
        assert!(
            !stdout.contains("pluginManifest=.codex/plugins/cache/asp-project"),
            "stdout={stdout}"
        );
        assert!(
            !stdout.contains("pluginSkill=.codex/plugins/cache/asp-project"),
            "stdout={stdout}"
        );
        assert_global_plugin_cache_refreshed(&root, &codex_home);
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
        let manifest_path = root
            .join("asp-codex-plugin")
            .join(".codex-plugin")
            .join("plugin.json");
        let hooks_path = root
            .join("asp-codex-plugin")
            .join("hooks")
            .join("hooks.json");
        std::fs::create_dir_all(manifest_path.parent().expect("plugin manifest dir"))
            .expect("create plugin manifest dir");
        std::fs::create_dir_all(hooks_path.parent().expect("plugin hooks dir"))
            .expect("create plugin hooks dir");
        std::fs::write(&manifest_path, "{}\n").expect("write plugin manifest");
        std::fs::write(&hooks_path, "{}\n").expect("write plugin hooks");
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

    fn assert_current_agent_config(root: &Path) {
        let agent_config_path = root.join(".agents").join("asp.toml");
        assert!(
            agent_config_path.is_file(),
            "missing canonical agent config under {}",
            agent_config_path.display()
        );
        let agent_config =
            std::fs::read_to_string(&agent_config_path).expect("read canonical agent config");
        assert!(
            agent_config.contains("[skills.agent-semantic-protocols]"),
            "{agent_config}"
        );
        assert!(
            agent_config.contains(
                "pluginSkill = \".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org\""
            ),
            "{agent_config}"
        );
        assert!(!agent_config.contains("aspOrg"), "{agent_config}");
        assert!(!agent_config.contains("orgArtifacts"), "{agent_config}");
        assert!(
            !agent_config.contains("[hook.agentOrgArtifacts]"),
            "{agent_config}"
        );
        assert!(!agent_config.contains("artifactsPath"), "{agent_config}");
        assert!(!agent_config.contains("entrySkillPath"), "{agent_config}");
    }

    fn assert_project_plugin_cache_refreshed(root: &Path) {
        assert_plugin_cache_refreshed(root, &project_plugin_cache_root(root));
    }

    fn assert_global_plugin_cache_refreshed(root: &Path, codex_home: &Path) {
        assert_plugin_cache_refreshed(root, &global_plugin_cache_root(codex_home));
    }

    fn assert_plugin_cache_refreshed(root: &Path, cache_root: &Path) {
        assert!(
            cache_root
                .join(".codex-plugin")
                .join("plugin.json")
                .is_file(),
            "missing plugin cache manifest under {}",
            cache_root.display()
        );
        assert!(
            cache_root.join("hooks").join("hooks.json").is_file(),
            "missing plugin cache hooks under {}",
            cache_root.display()
        );
        let hooks = std::fs::read_to_string(cache_root.join("hooks").join("hooks.json"))
            .expect("read plugin cache hooks");
        assert!(
            hooks.contains("asp hook permission-request --client codex"),
            "{hooks}"
        );
        assert!(
            hooks.contains("asp hook pre-tool --client codex"),
            "{hooks}"
        );
        assert!(!hooks.contains("direnv exec"), "{hooks}");
        assert!(!hooks.contains("asp-codex-hook"), "{hooks}");
        assert!(
            !root.join(".bin").join("asp-codex-hook").exists(),
            "legacy hook wrapper must not be generated"
        );
        let cache_skill_dir = cache_root.join("skills").join("agent-semantic-protocols");
        let cache_skill_path = cache_skill_dir.join("SKILL.org");
        let skill = std::fs::read_to_string(&cache_skill_path).expect("read plugin cache skill");
        assert_eq!(skill, format!("{}\n", ASP_ORG_SKILL_TEMPLATE.trim_end()));
        assert!(skill.contains("* ASP Org"), "{skill}");
        assert!(skill.contains(":SKILL_ID: asp-org"), "{skill}");
        assert!(skill.contains("asp paths --get orgStateSkill"), "{skill}");
        assert!(skill.contains("asp paths --get orgArtifacts"), "{skill}");
        assert!(!skill.contains(&root.display().to_string()), "{skill}");
    }

    fn project_plugin_cache_root(root: &Path) -> PathBuf {
        root.join(".codex")
            .join("plugins")
            .join("cache")
            .join("asp-project")
            .join("asp-codex-plugin")
            .join("0.1.0")
    }

    fn global_plugin_cache_root(codex_home: &Path) -> PathBuf {
        codex_home
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

    fn prepend_path(bin_dir: &Path) -> String {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
        paths.insert(0, asp_bin_dir());
        paths.insert(0, bin_dir.to_path_buf());
        std::env::join_paths(paths)
            .expect("join PATH")
            .to_string_lossy()
            .into_owned()
    }

    fn asp_bin_dir() -> PathBuf {
        Path::new(env!("CARGO_BIN_EXE_asp"))
            .parent()
            .expect("CARGO_BIN_EXE_asp has parent")
            .to_path_buf()
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
    let asp_bin_dir = std::path::Path::new(env!("CARGO_BIN_EXE_asp"))
        .parent()
        .expect("CARGO_BIN_EXE_asp has parent")
        .to_path_buf();
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
    paths.insert(0, asp_bin_dir.clone());
    let path = std::env::join_paths(paths).expect("join PATH");
    std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .env("PATH", path)
        .env("SEMANTIC_AGENT_BIN_DIR", &asp_bin_dir)
        .env("ASP_STATE_HOME", root.join(".state"))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "hook", "--client", "claude", "."])
        .output()
        .expect("run claude hook install")
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
