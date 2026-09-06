use super::RemoteUrl;
use super::select_canonical_remote;

use super::GitIdentity;
use std::path::Path;
use std::process::Command;

#[test]
fn canonical_remote_identity_is_scheme_independent() {
    let expected = Some("github.com/tao3k/agent-semantic-protocols".to_string());
    assert_eq!(
        RemoteUrl("git@github.com:tao3k/agent-semantic-protocols.git".to_string())
            .canonical_identity(),
        expected
    );
    assert_eq!(
        RemoteUrl("ssh://git@github.com/tao3k/agent-semantic-protocols.git".to_string())
            .canonical_identity(),
        expected
    );
    assert_eq!(
        RemoteUrl("https://github.com/tao3k/agent-semantic-protocols.git/".to_string())
            .canonical_identity(),
        expected
    );
}

#[test]
fn local_remote_is_not_a_durable_global_identity() {
    assert_eq!(
        RemoteUrl("../local-repository.git".to_string()).canonical_identity(),
        None
    );
    assert_eq!(
        RemoteUrl("file:///tmp/local-repository.git".to_string()).canonical_identity(),
        None
    );
}

#[test]
fn enterprise_hostname_and_explicit_port_are_durable_identities() {
    assert_eq!(
        RemoteUrl("ssh://git@forge/team/repo.git".to_string()).canonical_identity(),
        Some("forge/team/repo".to_string())
    );
    assert_eq!(
        RemoteUrl("ssh://git@forge:2222/team/repo.git".to_string()).canonical_identity(),
        Some("forge:2222/team/repo".to_string())
    );
}

#[test]
fn canonical_remote_selection_is_explicit_for_multiple_remotes() {
    let remotes = vec![
        (
            "origin".to_string(),
            "https://example.com/fork/repo.git".to_string(),
        ),
        (
            "upstream".to_string(),
            "https://example.com/source/repo.git".to_string(),
        ),
    ];
    assert_eq!(
        select_canonical_remote(Some("upstream"), None, &remotes),
        Some("https://example.com/source/repo.git".to_string())
    );
    assert_eq!(
        select_canonical_remote(None, Some("upstream"), &remotes),
        Some("https://example.com/source/repo.git".to_string())
    );
    assert_eq!(
        select_canonical_remote(None, None, &remotes),
        Some("https://example.com/fork/repo.git".to_string())
    );
}

#[test]
fn one_non_origin_remote_is_unambiguous() {
    let remotes = vec![(
        "upstream".to_string(),
        "https://example.com/source/repo.git".to_string(),
    )];
    assert_eq!(
        select_canonical_remote(None, None, &remotes),
        Some("https://example.com/source/repo.git".to_string())
    );
}

#[test]
fn linked_worktrees_share_gix_repository_identity() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-gix-worktree-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos()
    ));
    let main = fixture.join("main");
    let linked = fixture.join("linked");
    std::fs::create_dir_all(&main).expect("create main worktree");

    let run_git = |cwd: &Path, args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("run git fixture command");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run_git(&main, &["init"]);
    run_git(
        &main,
        &[
            "-c",
            "user.name=ASP Test",
            "-c",
            "user.email=asp@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
    );
    run_git(
        &main,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:tao3k/agent-semantic-protocols.git",
        ],
    );
    run_git(
        &main,
        &[
            "worktree",
            "add",
            linked.to_str().expect("UTF-8 fixture path"),
            "-b",
            "linked",
        ],
    );

    let main_identity = GitIdentity::discover(&main);
    let linked_identity = GitIdentity::discover(&linked);
    assert_ne!(main_identity.toplevel, linked_identity.toplevel);
    assert_ne!(main_identity.git_dir, linked_identity.git_dir);
    assert_eq!(main_identity.common_git_dir, linked_identity.common_git_dir);
    assert_eq!(main_identity.remote_url, linked_identity.remote_url);

    let state_home = fixture.join("state");
    let main_state = crate::state_core::ResolvedState::resolve_with_state_home(&main, &state_home)
        .expect("resolve main worktree state");
    let linked_state =
        crate::state_core::ResolvedState::resolve_with_state_home(&linked, &state_home)
            .expect("resolve linked worktree state");
    let package_entry = main.join("crates/example");
    std::fs::create_dir_all(&package_entry).expect("create package entry directory");
    std::fs::write(
        package_entry.join("Cargo.toml"),
        "[package]\nname = \"example\"\n",
    )
    .expect("write package entry");
    let package_state =
        crate::state_core::ResolvedState::resolve_with_state_home(&package_entry, &state_home)
            .expect("resolve package entry state");
    assert_eq!(main_state.repo.repo_id, linked_state.repo.repo_id);
    assert_eq!(
        main_state.workspace.workspace_id, package_state.workspace.workspace_id,
        "package/project entries inside one worktree must remain one workspace"
    );
    assert_ne!(
        main_state.workspace.workspace_id,
        linked_state.workspace.workspace_id
    );

    for _ in 0..16 {
        main_state
            .ensure_workspace_state_layout()
            .expect("materialize main worktree state");
        linked_state
            .ensure_workspace_state_layout()
            .expect("materialize linked worktree state");
    }

    assert!(!state_home.join("projects/by-id").exists());
    let workspace_count = std::fs::read_dir(state_home.join("workspaces"))
        .expect("read workspace directories")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count();
    assert_eq!(
        workspace_count, 2,
        "linked worktrees must materialize two workspace directories"
    );

    std::fs::remove_dir_all(&fixture).expect("remove worktree fixture");
}
