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
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("[plugin-install]"),
            "stdout={}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_project_plugin_bundle_installed(&root);

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
        write_stale_plugin_skill_contract(&root);

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
            String::from_utf8_lossy(&output.stdout).contains("[plugin-install]"),
            "stdout={}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_project_plugin_bundle_installed(&root);

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

    fn assert_project_plugin_bundle_installed(root: &Path) {
        let plugin_root = root.join("asp-codex-plugin");
        assert!(
            plugin_root
                .join(".codex-plugin")
                .join("plugin.json")
                .is_file(),
            "missing plugin manifest under {}",
            plugin_root.display()
        );
        assert!(
            plugin_root.join("hooks").join("hooks.json").is_file(),
            "missing plugin hooks under {}",
            plugin_root.display()
        );
        let skill_dir = plugin_root.join("skills").join("agent-semantic-protocols");
        assert!(
            skill_dir.join("SKILL.org").is_file(),
            "missing plugin skill under {}",
            skill_dir.display()
        );
        assert!(
            !skill_dir.join("SKILL.contract.org").exists(),
            "plugin skill contract should remain repository-side validation input, not an installed user artifact under {}",
            skill_dir.display()
        );
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
