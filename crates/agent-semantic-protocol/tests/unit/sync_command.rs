#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sync_clones_org_state_from_git_repo_and_updates_preserving_flow() {
        let source = temp_root("org-source");
        init_org_repo(&source, "v1");
        let project = temp_root("sync-project");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");

        let first = run_asp_sync(&project, &source);
        assert!(
            first.contains("[asp-sync]"),
            "expected asp sync receipt, got {first}"
        );
        let org_state = project
            .join(".cache")
            .join("agent-semantic-protocol")
            .join("org");
        assert!(
            org_state.join(".git").is_dir(),
            "org state must be a git checkout"
        );
        assert_eq!(
            std::fs::read_to_string(org_state.join("skills").join("ASP_ORG.org"))
                .expect("read asp org skill"),
            "* ASP Org v1\n"
        );
        let org_artifacts = project
            .join(".cache")
            .join("agent-semantic-protocol")
            .join("artifacts")
            .join("org");
        assert!(org_artifacts.join("flow").join("plans").is_dir());
        assert!(org_artifacts.join("flow").join("sdd").is_dir());
        assert!(org_artifacts.join("flow").join("BDR").is_dir());
        let local_plan = org_artifacts.join("flow").join("plans").join("local.org");
        std::fs::write(&local_plan, "* Local plan\n").expect("write local flow file");

        update_org_repo(&source, "v2");
        let second = run_asp_sync(&project, &source);
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

        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn sync_refreshes_bundled_org_resources_even_when_state_is_dirty() {
        let project = temp_root("bundled-dirty-project");
        std::fs::create_dir_all(project.join(".git")).expect("create project git marker");
        let org_state = project
            .join(".cache")
            .join("agent-semantic-protocol")
            .join("org");
        std::fs::create_dir_all(org_state.join(".git")).expect("create dirty org git marker");
        std::fs::write(org_state.join("local-note.org"), "* local note\n")
            .expect("write local dirty note");

        let output = run_bundled_asp_sync(&project);
        assert!(
            output.contains("orgStatus=bundled-copied"),
            "expected bundled resource sync receipt, got {output}"
        );
        assert!(
            org_state
                .join("templates")
                .join("agent.task.v1.org")
                .is_file(),
            "asp sync must refresh templates from bundled languages/org"
        );
        assert!(
            org_state.join("local-note.org").is_file(),
            "asp sync must not remove non-resource local state"
        );
        assert!(
            project
                .join(".cache")
                .join("agent-semantic-protocol")
                .join("artifacts")
                .join("org")
                .join("flow")
                .join("plans")
                .is_dir(),
            "asp sync must keep creating org artifact flow dirs"
        );

        let _ = std::fs::remove_dir_all(project);
    }

    fn run_asp_sync(project: &Path, source: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(project)
            .env("ASP_ORG_REPO_URL", source)
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

    fn run_bundled_asp_sync(project: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(project)
            .env_remove("ASP_ORG_REPO_URL")
            .env("PRJ_CACHE_HOME", project.join(".cache"))
            .args(["sync"])
            .output()
            .expect("run bundled asp sync");
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
