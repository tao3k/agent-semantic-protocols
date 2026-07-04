#[cfg(unix)]
mod unix {
    use agent_semantic_runtime::state_core::ResolvedState;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sync_clones_org_state_from_git_repo_and_updates_preserving_flow() {
        let source = temp_root("org-source");
        init_org_repo(&source, "v1");
        let project = temp_root("sync-project");
        let state_home = temp_root("sync-state");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");

        let first = run_asp_sync(&project, &source, &state_home);
        assert!(
            first.contains("[asp-sync]"),
            "expected asp sync receipt, got {first}"
        );
        let org_state = state_home.join("org");
        assert!(
            org_state.join(".git").is_dir(),
            "org state must be a git checkout"
        );
        assert_eq!(
            std::fs::read_to_string(org_state.join("skills").join("ASP_ORG.org"))
                .expect("read asp org skill"),
            "* ASP Org v1\n"
        );
        let org_artifacts = ResolvedState::resolve_with_state_home(&project, &state_home)
            .expect("resolved state")
            .paths
            .artifacts_dir
            .join("org");
        assert!(org_artifacts.join("flow").join("plans").is_dir());
        assert!(org_artifacts.join("flow").join("sdd").is_dir());
        assert!(org_artifacts.join("flow").join("bdr").is_dir());
        let local_plan = org_artifacts.join("flow").join("plans").join("local.org");
        std::fs::write(&local_plan, "* Local plan\n").expect("write local flow file");

        update_org_repo(&source, "v2");
        let second = run_asp_sync(&project, &source, &state_home);
        assert!(
            second.contains("orgStatus=updated"),
            "expected fast-forward update receipt, got {second}"
        );
        assert_eq!(
            std::fs::read_to_string(org_state.join("skills").join("ASP_ORG.org"))
                .expect("read updated asp org skill"),
            "* ASP Org v2\n"
        );
        assert!(
            local_plan.is_file(),
            "sync must preserve untracked local flow state"
        );
        assert_eq!(
            git_output(&org_state, &["status", "--porcelain"]),
            "",
            "local flow state should be excluded from the backing org repo status"
        );
        assert!(
            !project.join(".cache").exists(),
            "sync must not materialize Org state under the project cache"
        );

        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(state_home);
    }

    #[test]
    fn sync_uses_languages_org_remote_by_default_without_copying_bundled_files() {
        let source = temp_root("default-org-source");
        init_org_repo(&source, "default");
        let git_config = temp_root("default-org-gitconfig");
        std::fs::write(
            &git_config,
            format!(
                "[url \"file://{}\"]\n\tinsteadOf = https://github.com/tao3k/org.git\n",
                source.display()
            ),
        )
        .expect("write git config");

        let project = temp_root("default-remote-project");
        let state_home = temp_root("default-remote-state");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");

        let output = run_default_remote_asp_sync(&project, &git_config, &state_home);
        assert!(
            output.contains("orgRepo=https://github.com/tao3k/org.git"),
            "expected default org remote receipt, got {output}"
        );
        assert!(
            output.contains("orgStatus=cloned"),
            "expected default remote clone receipt, got {output}"
        );
        assert!(
            !output.contains("copiedFiles="),
            "asp sync receipt must not expose copy semantics, got {output}"
        );
        let org_state = state_home.join("org");
        let org_artifacts = ResolvedState::resolve_with_state_home(&project, &state_home)
            .expect("resolved state")
            .paths
            .artifacts_dir
            .join("org");
        assert!(
            org_state.join(".git").is_dir(),
            "default sync must create a git checkout"
        );
        assert!(
            org_state.join("skills").join("ASP_ORG.org").is_file(),
            "asp sync must materialize org resources through git clone"
        );
        assert!(
            org_artifacts.join("flow").join("plans").is_dir(),
            "asp sync must keep creating org artifact flow dirs"
        );
        assert!(
            !project.join(".cache").exists(),
            "sync must not materialize Org state under the project cache"
        );

        let _ = std::fs::remove_file(git_config);
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(state_home);
    }

    #[test]
    fn sync_projects_global_agent_configs_to_host_agents() {
        let source = temp_root("org-source-agent-configs");
        init_org_repo(&source, "v1");
        let project = temp_root("sync-agent-config-project");
        let state_home = temp_root("sync-agent-config-state");
        let codex_home = temp_root("sync-agent-config-codex");
        let claude_home = temp_root("sync-agent-config-claude");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");
        let agents_dir = state_home.join("agents");
        std::fs::create_dir_all(&agents_dir).expect("create agents dir");
        let codex_source = agents_dir.join("asp-explorer_codex.toml");
        let claude_source = agents_dir.join("asp-explorer_claude.md");
        std::fs::write(&codex_source, "name = \"asp_explorer\"\nmodel = \"gpt-5.3-codex-spark\"\nsandbox_mode = \"read-only\"\n")
            .expect("write codex agent config");
        std::fs::write(&claude_source, "---\nname: asp-explorer\n---\n")
            .expect("write claude agent config");

        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["sync"])
            .output()
            .expect("run asp sync");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(
            stdout.contains("agentConfigs=2"),
            "expected two projected agent configs, got {stdout}"
        );
        assert_eq!(
            std::fs::read_link(codex_home.join("agents").join("asp-explorer.toml"))
                .expect("codex agent symlink")
                .canonicalize()
                .expect("codex agent symlink"),
            codex_source.canonicalize().expect("codex source")
        );
        assert_eq!(
            std::fs::read_link(claude_home.join("agents").join("asp-explorer.md"))
                .expect("claude agent symlink")
                .canonicalize()
                .expect("claude agent symlink"),
            claude_source.canonicalize().expect("claude source")
        );

        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(state_home);
        let _ = std::fs::remove_dir_all(codex_home);
        let _ = std::fs::remove_dir_all(claude_home);
    }

    fn run_asp_sync(project: &Path, source: &Path, state_home: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(project)
            .env("ASP_ORG_REPO_URL", source)
            .env("ASP_STATE_HOME", state_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["sync"])
            .output()
            .expect("run asp sync");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 stdout")
    }

    fn run_default_remote_asp_sync(project: &Path, git_config: &Path, state_home: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(project)
            .env_remove("ASP_ORG_REPO_URL")
            .env("GIT_CONFIG_GLOBAL", git_config)
            .env("ASP_STATE_HOME", state_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["sync"])
            .output()
            .expect("run default remote asp sync");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 stdout")
    }

    fn init_org_repo(root: &Path, version: &str) {
        std::fs::create_dir_all(root).expect("create source repo");
        run_git(root, &["init", "-q"]);
        write_org_resources(root, version);
        run_git(root, &["add", "."]);
        run_git(
            root,
            &[
                "-c",
                "user.name=ASP Test",
                "-c",
                "user.email=asp-test@example.com",
                "commit",
                "-q",
                "-m",
                "initial org resources",
            ],
        );
    }

    fn update_org_repo(root: &Path, version: &str) {
        write_org_resources(root, version);
        run_git(root, &["add", "."]);
        run_git(
            root,
            &[
                "-c",
                "user.name=ASP Test",
                "-c",
                "user.email=asp-test@example.com",
                "commit",
                "-q",
                "-m",
                "update org resources",
            ],
        );
    }

    fn write_org_resources(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("contracts")).expect("create contracts");
        std::fs::create_dir_all(root.join("templates")).expect("create templates");
        std::fs::create_dir_all(root.join("skills")).expect("create skills");
        std::fs::write(
            root.join("skills").join("ASP_ORG.org"),
            format!("* ASP Org {version}\n"),
        )
        .expect("write asp org skill");
        std::fs::write(root.join("templates").join("agent.plan.v1.org"), "* Plan\n")
            .expect("write plan template");
        std::fs::write(
            root.join("contracts").join("agent.plan.v1.org"),
            "* Contract\n",
        )
        .expect("write contract");
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    fn git_output(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 stdout")
    }

    fn temp_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("asp-sync-{name}-{unique}"));
        let _ = std::fs::remove_dir_all(&root);
        root
    }
}
