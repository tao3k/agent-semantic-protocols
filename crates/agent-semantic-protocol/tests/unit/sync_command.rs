#[cfg(unix)]
mod unix {
    use agent_semantic_runtime::state_core::ResolvedState;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sync_does_not_clone_or_pull_org_state() {
        let source = temp_root("org-source");
        init_org_repo(&source, "v1");
        let project = temp_root("sync-project");
        let state_home = temp_root("sync-state");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");

        let first = run_asp_sync(&project, &source, &state_home);
        assert!(
            first.contains("[asp-agent-config-sync]"),
            "expected asp agent config sync receipt, got {first}"
        );
        assert!(first.contains("scope=global-agent-config"));
        assert!(first.contains("trigger=explicit"));
        assert!(first.contains("orgStateSync=consumer-lazy"));
        assert!(first.contains("gitPulls=0"));
        assert!(first.contains("gitFetches=0"));
        assert!(first.contains("gitClones=0"));
        let org_state = state_home.join("org");
        assert!(
            !org_state.exists(),
            "generic sync must not materialize the private Org checkout"
        );

        update_org_repo(&source, "v2");
        let second = run_asp_sync(&project, &source, &state_home);
        assert!(
            second.contains("orgStateSync=consumer-lazy")
                && second.contains("gitPulls=0")
                && second.contains("gitFetches=0")
                && second.contains("gitClones=0"),
            "repeated generic sync must remain a zero-Git writer, got {second}"
        );
        assert!(!org_state.exists());
        assert!(
            !project.join(".cache").exists(),
            "sync must not materialize Org state under the project cache"
        );

        let workspace_argument = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_STATE_HOME", &state_home)
            .args(["agent", "config", "sync", "."])
            .output()
            .expect("run asp agent config sync with a positional argument");
        assert!(
            !workspace_argument.status.success(),
            "agent config sync must reject positional arguments"
        );
        let stderr = String::from_utf8_lossy(&workspace_argument.stderr);
        assert!(
            stderr.contains("does not accept positional arguments"),
            "expected an actionable responsibility error, got {stderr}"
        );

        let root_sync = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .args(["sync"])
            .output()
            .expect("run removed root sync surface");
        assert!(
            !root_sync.status.success(),
            "root sync must fail closed instead of choosing a domain"
        );
        let root_sync_stderr = String::from_utf8_lossy(&root_sync.stderr);
        assert!(root_sync_stderr.contains("has no cross-domain responsibility"));
        assert!(root_sync_stderr.contains("asp agent config sync"));
        assert!(root_sync_stderr.contains("asp cache source-index rebuild"));

        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(state_home);
    }

    #[test]
    fn sync_leaves_default_org_remote_to_lazy_consumers() {
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
            output.contains("orgStateSync=consumer-lazy"),
            "expected lazy Org state receipt, got {output}"
        );
        assert!(
            output.contains("gitPulls=0")
                && output.contains("gitFetches=0")
                && output.contains("gitClones=0"),
            "generic sync must publish its zero-Git writer receipt, got {output}"
        );
        let org_state = state_home.join("org");
        let org_artifacts = ResolvedState::resolve_with_state_home(&project, &state_home)
            .expect("resolved state")
            .paths
            .artifacts_dir
            .join("org");
        assert!(
            !org_state.exists(),
            "generic sync must not create the private Org checkout"
        );
        assert!(
            !org_artifacts.exists(),
            "generic sync must leave Org artifacts to the consuming workflow"
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
        std::fs::write(
            agents_dir.join("config.toml"),
            r#"[agents.asp_explorer]
host_agent_name = "asp_explorer"
profile = "asp-explorer_codex.toml"
projection = "asp-explorer.toml"
"#,
        )
        .expect("write ASP agent registry");
        std::fs::create_dir_all(&codex_home).expect("create Codex home");
        std::fs::write(
            codex_home.join("config.toml"),
            "model = \"gpt-5.6\"\n\n[agents]\nmax_threads = 4\n\n[features]\nmulti_agent_v2 = true\nunified_exec = true\n",
        )
        .expect("write legacy Codex feature config");

        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
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
        assert!(
            stdout.contains("codexAgentRegistry=1"),
            "expected one Codex registry entry, got {stdout}"
        );
        assert!(
            stdout.contains("codexSpawnAgentMetadata=visible-agent-type"),
            "expected visible Codex agent_type projection, got {stdout}"
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
        let codex_config =
            std::fs::read_to_string(codex_home.join("config.toml")).expect("Codex config");
        assert!(codex_config.contains("# BEGIN ASP MANAGED CODEX AGENT REGISTRY"));
        assert!(codex_config.contains("[features.multi_agent_v2]"));
        assert!(codex_config.contains("enabled = true"));
        assert!(codex_config.contains("hide_spawn_agent_metadata = false"));
        assert!(codex_config.contains("tool_namespace = \"collaboration_v2\""));
        assert!(!codex_config.contains("expose_spawn_agent_model_overrides"));
        assert!(!codex_config.contains("multi_agent_v2 = true"));
        assert!(!codex_config.contains("max_threads = 4"));
        assert!(codex_config.contains("unified_exec = true"));
        assert!(codex_config.contains("model = \"gpt-5.6\""));
        assert!(codex_config.contains("[agents.asp_explorer]"));
        assert!(codex_config.contains("config_file = \"agents/asp-explorer.toml\""));
        let second_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("rerun asp sync");
        assert!(second_output.status.success());
        assert_eq!(
            std::fs::read_to_string(codex_home.join("config.toml"))
                .expect("Codex config after second sync"),
            codex_config,
            "Codex config projection must be idempotent"
        );
        let config_with_hidden_metadata = codex_config.replace(
            "hide_spawn_agent_metadata = false",
            "hide_spawn_agent_metadata = true\nexpose_spawn_agent_model_overrides = true",
        );
        std::fs::write(codex_home.join("config.toml"), config_with_hidden_metadata)
            .expect("write drifted Codex multi-agent v2 config");
        let repair_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("repair drifted Codex multi-agent v2 config");
        assert!(repair_output.status.success());
        let repaired_codex_config =
            std::fs::read_to_string(codex_home.join("config.toml")).expect("repaired Codex config");
        assert!(repaired_codex_config.contains("hide_spawn_agent_metadata = false"));
        assert!(repaired_codex_config.contains("tool_namespace = \"collaboration_v2\""));
        assert!(!repaired_codex_config.contains("expose_spawn_agent_model_overrides"));
        let parsed_codex_config: toml::Value =
            toml::from_str(&repaired_codex_config).expect("parse repaired Codex config");
        assert_eq!(
            parsed_codex_config["features"]["multi_agent_v2"]["hide_spawn_agent_metadata"]
                .as_bool(),
            Some(false)
        );
        assert_eq!(
            parsed_codex_config["features"]["multi_agent_v2"]["tool_namespace"].as_str(),
            Some("collaboration_v2")
        );
        let legacy_namespace_config = repaired_codex_config.replace(
            "tool_namespace = \"collaboration_v2\"",
            "tool_namespace = \"asp_collaboration\"",
        );
        std::fs::write(codex_home.join("config.toml"), legacy_namespace_config)
            .expect("write legacy ASP namespace");
        let migrate_namespace_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("migrate legacy ASP namespace");
        assert!(migrate_namespace_output.status.success());
        let migrated_namespace_config = std::fs::read_to_string(codex_home.join("config.toml"))
            .expect("migrated namespace config");
        assert!(migrated_namespace_config.contains("tool_namespace = \"collaboration_v2\""));
        assert!(!migrated_namespace_config.contains("tool_namespace = \"asp_collaboration\""));

        let user_namespace_config = migrated_namespace_config.replace(
            "tool_namespace = \"collaboration_v2\"",
            "tool_namespace = \"my_collaboration_tools\"",
        );
        std::fs::write(codex_home.join("config.toml"), &user_namespace_config)
            .expect("write user-owned namespace");
        let preserve_namespace_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&project)
            .env("ASP_ORG_REPO_URL", &source)
            .env("ASP_STATE_HOME", &state_home)
            .env("CODEX_HOME", &codex_home)
            .env("CLAUDE_HOME", &claude_home)
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("preserve user-owned namespace");
        assert!(preserve_namespace_output.status.success());
        assert_eq!(
            std::fs::read_to_string(codex_home.join("config.toml"))
                .expect("user-owned namespace config after sync"),
            user_namespace_config,
            "asp sync must not overwrite a user-owned tool namespace"
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
            .env("CODEX_HOME", state_home.join("codex-home"))
            .env("CLAUDE_HOME", state_home.join("claude-home"))
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("run asp agent config sync");
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
            .env("CODEX_HOME", state_home.join("codex-home"))
            .env("CLAUDE_HOME", state_home.join("claude-home"))
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["agent", "config", "sync"])
            .output()
            .expect("run asp agent config sync with default Org remote");
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
#[test]
fn sync_command_has_zero_runtime_database_or_activation_authority() {
    let source = include_str!("../../src/command/sync.rs");
    for forbidden in [
        "ClientDbEngine::",
        "AgentSessionRegistry::",
        "load_or_refresh_default_activation",
        "load_or_sync_activation_for_language",
        "run_org_state_sync",
    ] {
        assert!(
            !source.contains(forbidden),
            "asp sync must not regain runtime authority through {forbidden}"
        );
    }
    for required in [
        "activationWrites=0",
        "dbOpens=0",
        "dbTransactions=0",
        "sessionRegistryOpens=0",
        "orgStateSync=consumer-lazy",
        "gitPulls=0",
        "gitFetches=0",
        "gitClones=0",
    ] {
        assert!(
            source.contains(required),
            "asp sync must publish the zero-authority receipt field {required}"
        );
    }
}
#[cfg(unix)]
#[test]
fn sync_publishes_workspace_agent_profiles_before_host_projection() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-sync-workspace-profiles-{}-{nonce}",
        std::process::id()
    ));
    let workspace = root.join("workspace");
    let workspace_agents = workspace.join("agents");
    let state_home = root.join("state");
    let codex_home = root.join("codex");
    let claude_home = root.join("claude");
    std::fs::create_dir_all(&workspace_agents).expect("workspace agents");
    let profile = concat!(
        "name = \"workspace_testing\"\n",
        "description = \"Workspace testing lane.\"\n",
        "model = \"gpt-5.6-luna\"\n",
        "model_reasoning_effort = \"low\"\n",
        "sandbox_mode = \"workspace-write\"\n",
        "developer_instructions = \"Run tests.\"\n",
    );
    let workspace_profile = workspace_agents.join("workspace-testing_codex.toml");
    std::fs::write(&workspace_profile, profile).expect("workspace profile");
    std::fs::write(
        workspace_agents.join("README.org"),
        "not a managed profile\n",
    )
    .expect("workspace readme");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["agent", "config", "sync"])
        .current_dir(&workspace)
        .env("ASP_STATE_HOME", &state_home)
        .env("CODEX_HOME", &codex_home)
        .env("CLAUDE_HOME", &claude_home)
        .env("HOME", &root)
        .output()
        .expect("run asp agent config sync");

    assert!(
        output.status.success(),
        "sync failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("publishedAgentConfigs=1"), "{stdout}");
    let global_profile = state_home
        .join("agents")
        .join("workspace-testing_codex.toml");
    assert_eq!(
        std::fs::read_to_string(&global_profile).expect("global profile"),
        profile
    );
    assert!(!state_home.join("agents").join("README.org").exists());
    let projection = codex_home.join("agents").join("workspace-testing.toml");
    assert_eq!(
        std::fs::read_link(&projection).expect("Codex projection symlink"),
        global_profile
    );
    assert_eq!(
        std::fs::read_to_string(&projection).expect("projected profile"),
        profile
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}
