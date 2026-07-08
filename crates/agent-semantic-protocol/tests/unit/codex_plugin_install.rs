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
            .env("ASP_STATE_HOME", root.join(".state"))
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
        assert!(stdout.contains("[plugin-install]"), "stdout={stdout}");
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
            !project_config.contains("\"$repo_root/.bin/asp\" hook"),
            "{project_config}"
        );
        assert!(
            !project_config.contains("[agents.asp_explorer]"),
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
            global_config.contains("# BEGIN agent-semantic-protocol agent hooks"),
            "{global_config}"
        );
        assert!(
            global_config.contains("[[hooks.pre_tool_use]]"),
            "{global_config}"
        );
        assert!(global_config.contains("direnv exec"), "{global_config}");
        assert!(global_config.contains(" hook pre-tool "), "{global_config}");
        assert!(
            global_config.contains(r#"repo_root="${CODEX_WORKSPACE_ROOT:-${PWD:-.}}""#),
            "{global_config}"
        );
        assert!(
            !global_config.contains("\"$repo_root/.bin/asp\" hook"),
            "{global_config}"
        );
        assert!(
            global_config.contains(
                r#"nickname_candidates = ["ASP Explore", "ASP Reasoning", "ASP Search"]"#
            ),
            "{global_config}"
        );
        assert!(
            global_config.contains(".codex-home/config.toml:pre_tool_use:0:0"),
            "{global_config}"
        );
        let agent_config = std::fs::read_to_string(root.join(".agents").join("asp.toml"))
            .expect("read agent config");
        assert!(agent_config.contains("[providers.org]"), "{agent_config}");
        assert!(agent_config.contains("enabled = false"), "{agent_config}");

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
            .args(["install", "plugin", "--codex", "."])
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
        assert_project_plugin_cache_refreshed(&root);
        assert_global_plugin_cache_refreshed(&root, &codex_home);

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
        assert!(hooks.contains("/.bin/asp-codex-hook"), "{hooks}");
        assert!(!hooks.contains("direnv exec . asp hook"), "{hooks}");
        let wrapper = std::fs::read_to_string(root.join(".bin").join("asp-codex-hook"))
            .expect("read hook wrapper");
        assert!(wrapper.contains("direnv exec "), "{wrapper}");
        assert!(wrapper.contains("/.bin/asp hook"), "{wrapper}");
        assert!(wrapper.contains("2>/dev/null"), "{wrapper}");
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

    fn temp_project_root(name: &str) -> PathBuf {
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
        paths.insert(0, bin_dir.to_path_buf());
        std::env::join_paths(paths)
            .expect("join PATH")
            .to_string_lossy()
            .into_owned()
    }
}
