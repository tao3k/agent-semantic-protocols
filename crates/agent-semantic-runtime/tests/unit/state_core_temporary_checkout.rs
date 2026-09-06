use super::CheckoutIdentity;
use super::RepoIdentity;
use super::RepoPersistence;
use super::is_temporary_checkout_path;
use crate::git::GitIdentity;
use crate::git::RemoteUrl;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn recognizes_operating_system_temporary_roots() {
    assert!(is_temporary_checkout_path(Path::new("/tmp/asp-fixture")));
    assert!(is_temporary_checkout_path(Path::new(
        "/private/tmp/asp-fixture"
    )));
    assert!(is_temporary_checkout_path(Path::new(
        "/private/var/folders/aa/bb/T/asp-fixture"
    )));
    assert!(!is_temporary_checkout_path(Path::new(
        "/Users/example/src/project"
    )));
}

#[test]
fn standalone_temporary_git_repository_is_ephemeral() {
    let checkout = CheckoutIdentity {
        root: PathBuf::from("/private/var/folders/aa/bb/T/fixture"),
        display_name: "fixture".to_string(),
    };
    let git = GitIdentity {
        toplevel: Some(checkout.root.clone()),
        git_dir: Some(checkout.root.join(".git")),
        common_git_dir: Some(checkout.root.join(".git")),
        remote_url: None,
    };

    let repo = RepoIdentity::from_checkout(&git, &checkout);

    assert_eq!(repo.persistence, RepoPersistence::EphemeralPath);
    assert!(repo.identity_basis.starts_with("git-common-dir:"));
}

#[test]
fn temporary_linked_worktree_inherits_non_temporary_common_git_identity() {
    let checkout = CheckoutIdentity {
        root: PathBuf::from("/private/var/folders/aa/bb/T/linked"),
        display_name: "linked".to_string(),
    };
    let common_git_dir = PathBuf::from("/Users/example/src/project/.git");
    let git = GitIdentity {
        toplevel: Some(checkout.root.clone()),
        git_dir: Some(common_git_dir.join("worktrees/linked")),
        common_git_dir: Some(common_git_dir.clone()),
        remote_url: None,
    };

    let repo = RepoIdentity::from_checkout(&git, &checkout);

    assert_eq!(repo.persistence, RepoPersistence::Git);
    assert_eq!(
        repo.identity_basis,
        format!("git-common-dir:{}", common_git_dir.display())
    );
}

#[test]
fn canonical_remote_admits_durability_without_becoming_repository_identity() {
    let checkout = CheckoutIdentity {
        root: PathBuf::from("/tmp/remote-workspace"),
        display_name: "remote-workspace".to_string(),
    };
    let git = GitIdentity {
        toplevel: Some(checkout.root.clone()),
        git_dir: Some(checkout.root.join(".git")),
        common_git_dir: Some(checkout.root.join(".git")),
        remote_url: Some(RemoteUrl(
            "git@github.com:tao3k/agent-semantic-protocols.git".to_string(),
        )),
    };

    let repo = RepoIdentity::from_checkout(&git, &checkout);

    assert_eq!(repo.persistence, RepoPersistence::Git);
    assert_eq!(
        repo.identity_basis,
        format!("git-common-dir:{}", checkout.root.join(".git").display())
    );
}

#[test]
fn gix_discovers_a_real_linked_worktree_as_a_workspace_of_its_owner_project() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-gix-linked-worktree-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let owner = fixture.join("owner");
    let linked = fixture.join("linked");
    let state_home = fixture.join("state");
    fs::create_dir_all(&owner).expect("create owner checkout");
    run_git(&owner, &["init", "--quiet"]);
    run_git(&owner, &["config", "user.email", "asp@example.invalid"]);
    run_git(&owner, &["config", "user.name", "ASP Test"]);
    run_git(
        &owner,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/gix-linked-worktree.git",
        ],
    );
    fs::write(owner.join("tracked.txt"), b"tracked\n").expect("write tracked fixture");
    run_git(&owner, &["add", "tracked.txt"]);
    run_git(&owner, &["commit", "--quiet", "-m", "fixture"]);
    run_git(
        &owner,
        &[
            "worktree",
            "add",
            "--quiet",
            linked.to_str().expect("UTF-8 worktree path"),
            "-b",
            "linked-fixture",
        ],
    );

    let owner_state =
        crate::state_core::ResolvedState::resolve_with_state_home(&owner, &state_home)
            .expect("Gix resolves owner");
    let linked_state =
        crate::state_core::ResolvedState::resolve_with_state_home(&linked, &state_home)
            .expect("Gix resolves linked worktree");

    assert_eq!(linked_state.repo.repo_id, owner_state.repo.repo_id);
    assert_ne!(
        linked_state.workspace.workspace_id,
        owner_state.workspace.workspace_id
    );
    assert!(linked_state.workspace.lifecycle.is_temporary());
    assert_ne!(
        linked_state.workspace.git_dir,
        owner_state.workspace.git_dir
    );
    fs::remove_dir_all(&fixture).expect("remove linked-worktree fixture");
}

#[test]
fn owner_bound_temporary_workspace_rejects_a_different_gix_project() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-owner-bound-mismatch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let durable_fixture = durable_fixture_root("owner-bound-mismatch");
    let owner = durable_fixture.join("owner");
    let temporary = fixture.join("temporary");
    let state_home = fixture.join("state");
    fs::create_dir_all(&owner).expect("create owner checkout");
    fs::create_dir_all(&temporary).expect("create temporary checkout");
    run_git(&owner, &["init", "--quiet"]);
    run_git(
        &owner,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/owner.git",
        ],
    );
    run_git(&temporary, &["init", "--quiet"]);
    run_git(
        &temporary,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/asp/different.git",
        ],
    );

    let error =
        crate::state_core::ResolvedState::resolve_temporary_workspace_with_owner_and_state_home(
            &temporary,
            &owner,
            &state_home,
        )
        .expect_err("Gix identity mismatch must be rejected");

    assert!(error.contains("Gix resolved temporary checkout"));
    assert!(!state_home.exists());
    fs::remove_dir_all(&fixture).expect("remove mismatch fixture");
    fs::remove_dir_all(&durable_fixture).expect("remove durable owner fixture");
}

fn durable_fixture_root(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/state-core-fixtures")
        .join(format!("{label}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&root).expect("create durable state fixture");
    root
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("run Gix fixture setup through git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
