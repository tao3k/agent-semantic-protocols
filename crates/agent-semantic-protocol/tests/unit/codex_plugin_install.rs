#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cleans_retired_project_hook_config() {
        let root = temp_project_root("codex-plugin-cleans-retired-hooks");
        let codex_home = root.join(".codex-home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        let project_config = root.join(".codex/config.toml");
        std::fs::create_dir_all(project_config.parent().expect("project config parent"))
            .expect("create project config parent");
        std::fs::write(
            &project_config,
            format!(
                "model = \"gpt-5\"\n\n[marketplaces.asp-project]\nlast_updated = \"2026-06-15T00:57:21Z\"\nsource_type = \"local\"\nsource = \"{}\"\n\n{}\n",
                root.display(),
                agent_semantic_hook::codex_hook_block(&root)
            ),
        )
        .expect("write retired project hook config");

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
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
        assert_no_downstream_plugin_bundle(&root);
        assert_project_plugin_cache_refreshed(&root);

        let content = std::fs::read_to_string(&project_config).expect("read project config");
        assert!(content.contains("model = \"gpt-5\""), "{content}");
        assert!(content.contains("source = \".\""), "{content}");
        assert!(!content.contains("last_updated ="), "{content}");
        assert!(!content.contains(&root.display().to_string()), "{content}");
        assert!(content.contains("[agents.asp_explorer]"), "{content}");
        assert!(
            !content.contains(agent_semantic_hook::ROOT_BLOCK_BEGIN),
            "{content}"
        );
        assert!(!content.contains("[[hooks."), "{content}");
        assert!(!content.contains("asp hook "), "{content}");

        let user_config =
            std::fs::read_to_string(codex_home.join("config.toml")).expect("read user config");
        assert!(user_config.contains("[projects."), "{user_config}");
        assert!(!user_config.contains("[hooks.state."), "{user_config}");

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    #[test]
    fn install_plugin_codex_runs_project_installer() {
        let root = temp_project_root("codex-plugin-unified-install");
        let codex_home = root.join(".codex-home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::write(root.join("asp.toml"), "[providers.org]\nenabled = false\n")
            .expect("write legacy asp.toml");
        write_stale_plugin_skill_contract(&root);
        write_stale_project_plugin_cache(&root);

        let fake_bin = write_fake_codex_cli(&root);
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("CODEX_HOME", &codex_home)
            .env("PATH", prepend_path(&fake_bin))
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
        assert_no_downstream_plugin_bundle(&root);
        assert_project_plugin_cache_refreshed(&root);
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

        std::fs::remove_dir_all(root).expect("cleanup temp project root");
    }

    fn write_stale_plugin_skill_contract(root: &Path) {
        let contract_path = root
            .join("asp-codex-plugin")
            .join("skills")
            .join("agent-semantic-protocols")
            .join("SKILL.contract.org");
        std::fs::create_dir_all(contract_path.parent().expect("plugin skill dir"))
            .expect("create plugin skill dir");
        std::fs::write(&contract_path, "* stale user-layer contract\n")
            .expect("write stale plugin skill contract");
    }

    fn write_stale_project_plugin_cache(root: &Path) {
        let cache_skill_path = project_plugin_cache_root(root)
            .join("skills")
            .join("agent-semantic-protocols")
            .join("SKILL.org");
        std::fs::create_dir_all(cache_skill_path.parent().expect("cache skill dir"))
            .expect("create cache skill dir");
        std::fs::write(
            &cache_skill_path,
            format!(
                "* ASP\n| REFER_ORG | ={}/.cache/agent-semantic-protocol/org/templates/ASP_ORG_SKILL.org#asp-org= |\n",
                root.display()
            ),
        )
        .expect("write stale plugin cache skill");
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

    fn assert_no_downstream_plugin_bundle(root: &Path) {
        let plugin_root = root.join("asp-codex-plugin");
        assert!(
            !plugin_root.exists(),
            "Codex plugin install must not write downstream plugin bundle under {}",
            plugin_root.display()
        );
        let project_skill_dir = root
            .join(".agents")
            .join("skills")
            .join("agent-semantic-protocols");
        assert!(
            !project_skill_dir.join("SKILL.org").exists(),
            "Codex plugin install must not write project SKILL.org under {}",
            project_skill_dir.display()
        );
        assert!(
            !project_skill_dir.join("SKILL.contract.org").exists(),
            "Codex plugin install must not write project SKILL.contract.org under {}",
            project_skill_dir.display()
        );
        assert!(
            !root.join("asp.toml").exists(),
            "legacy top-level asp.toml should be migrated away"
        );
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
            !agent_config.contains("template = \"SKILL.org\""),
            "{agent_config}"
        );
        assert!(
            agent_config.contains(
                "pluginSkill = \".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org\""
            ),
            "{agent_config}"
        );
        assert!(
            agent_config.contains("ASP_ORG_SKILL.org#asp-org"),
            "{agent_config}"
        );
        assert!(
            agent_config
                .contains("orgArtifacts = \".cache/agent-semantic-protocol/artifacts/org\""),
            "{agent_config}"
        );
        assert!(!agent_config.contains("orgSkill"), "{agent_config}");
        let marketplace_path = root
            .join(".agents")
            .join("plugins")
            .join("marketplace.json");
        assert!(
            !marketplace_path.exists(),
            "Codex plugin install must not write marketplace catalog under {}",
            marketplace_path.display()
        );
    }

    fn assert_project_plugin_cache_refreshed(root: &Path) {
        let cache_root = project_plugin_cache_root(root);
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
        let cache_skill_dir = cache_root.join("skills").join("agent-semantic-protocols");
        let cache_skill_path = cache_skill_dir.join("SKILL.org");
        let skill = std::fs::read_to_string(&cache_skill_path).expect("read plugin cache skill");
        assert!(skill.contains("* ASP Org"), "{skill}");
        assert!(skill.contains(":SKILL_ID: asp-org"), "{skill}");
        assert!(!skill.contains("SKILL.contract.org"), "{skill}");
        assert!(!skill.contains("Contract Assertions"), "{skill}");
        assert!(!skill.contains("asp-skill-has-root-heading"), "{skill}");
        assert!(
            !skill.contains("Generated from the repository root =SKILL.org="),
            "{skill}"
        );
        assert!(!skill.contains("#+CONTRACT_ORG:"), "{skill}");
        assert!(
            skill.contains(".cache/agent-semantic-protocol/org/templates/ASP_ORG_SKILL.org"),
            "{skill}"
        );
        assert!(
            skill.contains(".cache/agent-semantic-protocol/artifacts/org"),
            "{skill}"
        );
        assert!(!skill.contains(&root.display().to_string()), "{skill}");
        assert!(
            !cache_skill_dir.join("SKILL.contract.org").exists(),
            "plugin cache must not contain SKILL.contract.org under {}",
            cache_skill_dir.display()
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
