use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    RepositoryCandidateAuthority, RepositoryCandidateState, discover_repository_candidate_snapshot,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "asp-repository-candidates-{name}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, contents).expect("write fixture file");
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("run git fixture command");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn git_snapshot_is_deterministic_and_excludes_ignored_files() {
    let fixture = Fixture::new("tracked-untracked");
    fixture.git(&["init", "--quiet"]);
    fixture.write(".gitignore", "ignored/\n");
    fixture.write("src/tracked.rs", "pub fn tracked() {}\n");
    fixture.write("src/untracked.rs", "pub fn untracked() {}\n");
    fixture.write("ignored/generated.rs", "pub fn generated() {}\n");
    fixture.git(&["add", ".gitignore", "src/tracked.rs"]);

    let first = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover repository candidates")
        .expect("Git snapshot exists");
    let second = discover_repository_candidate_snapshot(&fixture.root)
        .expect("rediscover repository candidates")
        .expect("Git snapshot exists");
    let candidates = first
        .candidates
        .iter()
        .map(|candidate| (candidate.path.clone(), candidate.state, candidate.authority))
        .collect::<Vec<_>>();
    assert_eq!(
        candidates,
        vec![
            (
                PathBuf::from(".gitignore"),
                RepositoryCandidateState::Tracked,
                RepositoryCandidateAuthority::GitIndex
            ),
            (
                PathBuf::from("src/tracked.rs"),
                RepositoryCandidateState::Tracked,
                RepositoryCandidateAuthority::GitIndex
            ),
            (
                PathBuf::from("src/untracked.rs"),
                RepositoryCandidateState::Untracked,
                RepositoryCandidateAuthority::GitWorktree
            ),
        ]
    );
    assert_eq!(
        first.candidate_generation.digest,
        second.candidate_generation.digest
    );
    assert_eq!(
        first.candidate_scope.project_root,
        std::fs::canonicalize(&fixture.root).expect("canonical fixture root")
    );
    assert_eq!(first.metrics.index_entry_count, 2);
    assert_eq!(first.metrics.worktree_addition_count, 1);
    assert_eq!(first.metrics.candidate_count, 3);
    assert_eq!(first.metrics.full_workspace_reads, 0);
    assert_eq!(first.metrics.direct_db_opens, 0);
    assert!(
        !first
            .candidates
            .iter()
            .any(|candidate| candidate.path == Path::new("ignored/generated.rs"))
    );
}

#[test]
fn entirely_untracked_workspace_member_directory_is_expanded_to_source_files() {
    let fixture = Fixture::new("untracked-workspace-member");
    fixture.git(&["init", "--quiet"]);
    fixture.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/runtime-server\"]\nresolver = \"2\"\n",
    );
    fixture.git(&["add", "Cargo.toml"]);
    fixture.write(
        "crates/runtime-server/Cargo.toml",
        "[package]\nname = \"runtime-server\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    fixture.write(
        "crates/runtime-server/src/lib.rs",
        "pub fn runtime_server() {}\n",
    );

    let snapshot = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover repository candidates")
        .expect("Git snapshot exists");

    for expected in [
        "crates/runtime-server/Cargo.toml",
        "crates/runtime-server/src/lib.rs",
    ] {
        let candidate = snapshot
            .candidates
            .iter()
            .find(|candidate| candidate.path == Path::new(expected))
            .unwrap_or_else(|| {
                panic!("untracked workspace member source must be admitted: {expected}")
            });
        assert_eq!(candidate.state, RepositoryCandidateState::Untracked);
        assert_eq!(
            candidate.authority,
            RepositoryCandidateAuthority::GitWorktree
        );
    }
    assert_eq!(snapshot.metrics.worktree_addition_count, 2);
}

#[test]
fn repository_workspace_candidate_contains_runtime_server_source() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let started = std::time::Instant::now();
    let snapshot = discover_repository_candidate_snapshot(&workspace)
        .expect("discover repository workspace candidates")
        .expect("repository workspace has a Git candidate snapshot");
    let elapsed = started.elapsed();

    for expected in [
        "crates/agent-semantic-runtime-server/Cargo.toml",
        "crates/agent-semantic-runtime-server/src/runtime_asp_client.rs",
    ] {
        assert!(
            snapshot
                .candidates
                .iter()
                .any(|candidate| candidate.path == Path::new(expected)),
            "workspace candidate snapshot omitted {expected}"
        );
    }
    eprintln!(
        "[repository-candidate-workspace] candidateCount={} elapsedMicros={}",
        snapshot.candidates.len(),
        elapsed.as_micros()
    );
}

#[test]
fn tracked_same_path_content_edit_advances_candidate_generation() {
    let fixture = Fixture::new("tracked-content-edit");
    fixture.git(&["init", "--quiet"]);
    fixture.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    fixture.git(&["add", "src/lib.rs"]);
    fixture.git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "baseline",
    ]);
    let baseline = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover clean generation")
        .expect("Git snapshot exists");

    fixture.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
    let edited = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover edited generation")
        .expect("Git snapshot exists");

    assert_eq!(baseline.candidates, edited.candidates);
    assert_ne!(
        baseline.candidate_generation.digest, edited.candidate_generation.digest,
        "same-path tracked edits must invalidate the admitted generation"
    );
}

#[test]
fn dirty_nested_provider_checkout_does_not_block_parent_generation() {
    let provider = tempfile::tempdir().expect("provider repository");
    let provider_git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(provider.path())
            .output()
            .expect("run provider git command");
        assert!(
            output.status.success(),
            "provider git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    provider_git(&["init", "--quiet"]);
    std::fs::write(
        provider.path().join("provider.rs"),
        "pub fn provider() {}\n",
    )
    .expect("provider source");
    provider_git(&["add", "provider.rs"]);
    provider_git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "provider baseline",
    ]);

    let fixture = Fixture::new("dirty-provider-checkout");
    fixture.git(&["init", "--quiet"]);
    fixture.write("src/lib.rs", "pub fn workspace() {}\n");
    fixture.git(&["add", "src/lib.rs"]);
    fixture.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        provider.path().to_str().expect("UTF-8 provider path"),
        "languages/provider",
    ]);
    fixture.git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "workspace baseline",
    ]);
    fixture.write(
        "languages/provider/provider.rs",
        "pub fn provider_changed() {}\n",
    );

    let snapshot = discover_repository_candidate_snapshot(&fixture.root)
        .expect("dirty provider checkout must not block the parent generation")
        .expect("Git snapshot exists");
    assert!(
        snapshot
            .candidates
            .iter()
            .any(|candidate| candidate.path == Path::new("src/lib.rs"))
    );
    assert!(
        !snapshot
            .candidates
            .iter()
            .any(|candidate| candidate.path.starts_with("languages/provider")),
        "nested provider contents belong to their provider catalog, not the parent source scope"
    );
}

#[test]
fn sibling_workspace_edit_does_not_invalidate_scoped_candidate_generation() {
    let fixture = Fixture::new("scoped-content-edit");
    fixture.git(&["init", "--quiet"]);
    fixture.write("crates/a/src/lib.rs", "pub fn a() -> u8 { 1 }\n");
    fixture.write("crates/b/src/lib.rs", "pub fn b() -> u8 { 1 }\n");
    fixture.git(&["add", "crates/a/src/lib.rs", "crates/b/src/lib.rs"]);
    fixture.git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "baseline",
    ]);
    let workspace_a = fixture.root.join("crates/a");
    let baseline = discover_repository_candidate_snapshot(&workspace_a)
        .expect("discover workspace A")
        .expect("Git snapshot exists");

    fixture.write("crates/b/src/lib.rs", "pub fn b() -> u8 { 2 }\n");
    let sibling_edited = discover_repository_candidate_snapshot(&workspace_a)
        .expect("rediscover workspace A after sibling edit")
        .expect("Git snapshot exists");
    assert_eq!(
        baseline.candidate_generation.digest, sibling_edited.candidate_generation.digest,
        "sibling workspace changes must remain isolated"
    );

    fixture.write("crates/a/src/lib.rs", "pub fn a() -> u8 { 2 }\n");
    let local_edited = discover_repository_candidate_snapshot(&workspace_a)
        .expect("rediscover workspace A after local edit")
        .expect("Git snapshot exists");
    assert_ne!(
        sibling_edited.candidate_generation.digest, local_edited.candidate_generation.digest,
        "local workspace changes must advance its candidate generation"
    );
}

#[test]
fn git_snapshot_discovers_file_added_under_new_untracked_directory() {
    let fixture = Fixture::new("new-untracked-directory");
    fixture.git(&["init", "--quiet"]);
    fixture.write("Cargo.toml", "[workspace]\nmembers = []\n");
    fixture.git(&["add", "Cargo.toml"]);
    let before = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover initial repository candidates")
        .expect("Git snapshot exists");
    fixture.write("extra/new_usage.ss", "(def (new-scope-symbol) #t)\n");
    let after = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover changed repository candidates")
        .expect("Git snapshot exists");
    assert!(
        after
            .candidates
            .iter()
            .any(|candidate| candidate.path == PathBuf::from("extra/new_usage.ss"))
    );
    assert_ne!(
        before.candidate_generation.digest,
        after.candidate_generation.digest
    );
}

#[test]
fn checkout_identity_is_stable_while_head_advances_its_generation() {
    let fixture = Fixture::new("worktree-generation");
    fixture.git(&["init", "--quiet"]);
    fixture.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    fixture.git(&["add", "src/lib.rs"]);
    fixture.git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "first",
    ]);
    let first = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover primary checkout")
        .expect("Git snapshot exists");
    fixture.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
    fixture.git(&["add", "src/lib.rs"]);
    fixture.git(&[
        "-c",
        "user.name=ASP Test",
        "-c",
        "user.email=asp@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "second",
    ]);
    let second = discover_repository_candidate_snapshot(&fixture.root)
        .expect("rediscover primary checkout")
        .expect("Git snapshot exists");
    assert_eq!(
        first.worktree_identity.worktree_id,
        second.worktree_identity.worktree_id
    );
    assert_ne!(
        first.worktree_identity.head_id,
        second.worktree_identity.head_id
    );
    assert_ne!(
        first.candidate_generation.digest,
        second.candidate_generation.digest
    );
    let linked_root = fixture.root.with_extension("linked-worktree");
    let linked_root_arg = linked_root.to_string_lossy().into_owned();
    fixture.git(&[
        "worktree",
        "add",
        "--quiet",
        "--detach",
        &linked_root_arg,
        "HEAD~1",
    ]);
    let linked = discover_repository_candidate_snapshot(&linked_root)
        .expect("discover linked worktree")
        .expect("linked Git snapshot exists");
    assert_eq!(
        second.repository_identity.repository_id,
        linked.repository_identity.repository_id
    );
    assert_ne!(
        second.worktree_identity.worktree_id,
        linked.worktree_identity.worktree_id
    );
    fixture.git(&["worktree", "remove", "--force", &linked_root_arg]);
}

#[test]
fn nested_project_snapshot_rebases_candidates_to_project_root() {
    let fixture = Fixture::new("nested-project");
    fixture.git(&["init", "--quiet"]);
    fixture.write("Cargo.toml", "[workspace]\nmembers = [\"crates/leaf\"]\n");
    fixture.write(
        "crates/leaf/Cargo.toml",
        "[package]\nname = \"leaf\"\nversion = \"0.1.0\"\n",
    );
    fixture.write("crates/leaf/src/lib.rs", "pub fn leaf() {}\n");
    fixture.write("crates/sibling/src/lib.rs", "pub fn sibling() {}\n");
    fixture.git(&["add", "."]);
    let project_root = fixture.root.join("crates/leaf");
    let repository_snapshot = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover repository candidates")
        .expect("Git snapshot exists");
    let projected = repository_snapshot
        .scoped_to_project_root(&project_root)
        .expect("project repository candidates");
    let snapshot = discover_repository_candidate_snapshot(&project_root)
        .expect("discover nested repository candidates")
        .expect("Git snapshot exists");
    assert_eq!(projected, snapshot);
    assert_eq!(
        snapshot.candidate_scope.project_root,
        std::fs::canonicalize(&project_root).expect("canonical nested project root")
    );
    assert_eq!(
        snapshot
            .candidates
            .iter()
            .map(|candidate| candidate.path.as_path())
            .collect::<Vec<_>>(),
        vec![Path::new("Cargo.toml"), Path::new("src/lib.rs")]
    );
    assert!(
        snapshot
            .candidates
            .iter()
            .all(|candidate| !candidate.path.starts_with("crates/leaf"))
    );
}

#[test]
fn non_git_directory_has_no_repository_candidate_snapshot() {
    let fixture = Fixture::new("non-git");
    let snapshot =
        discover_repository_candidate_snapshot(&fixture.root).expect("discover non-Git directory");
    assert!(snapshot.is_none());
}

#[test]
fn asp_discovery_policy_is_typed_without_hiding_git_candidates() {
    let fixture = Fixture::new("asp-policy");
    fixture.git(&["init", "--quiet"]);
    fixture.write(
        "asp.toml",
        "[discovery]\nignoredDirNames = [\"generated\"]\n",
    );
    fixture.write(".agents/asp.toml", "[discovery]\nignoredDirNames = [\"vendor\", \".hidden\"]\nincludeHiddenDirNames = [\".hidden\"]\n");
    fixture.write("generated/explicit.rs", "pub fn explicit() {}\n");
    fixture.write("vendor/excluded.rs", "pub fn excluded() {}\n");
    fixture.write(".hidden/included.rs", "pub fn included() {}\n");
    fixture.git(&["add", "."]);
    let snapshot = discover_repository_candidate_snapshot(&fixture.root)
        .expect("discover repository candidates")
        .expect("Git snapshot exists");
    assert!(
        snapshot
            .candidates
            .iter()
            .any(|candidate| candidate.path == Path::new("vendor/excluded.rs"))
    );
    assert_eq!(
        snapshot
            .policy_exclusions
            .iter()
            .map(|exclusion| (
                exclusion.path.as_path(),
                exclusion.authority.as_str(),
                exclusion.reason_kind.as_str(),
                exclusion.matched_value.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(
            Path::new("vendor/excluded.rs"),
            "user-policy",
            "ignored-dir-name",
            "vendor"
        )]
    );
    assert_eq!(snapshot.metrics.policy_exclusion_count, 1);
    assert!(snapshot.policy_overlay_digest.starts_with("blake3:"));
}

#[test]
fn invalid_asp_discovery_policy_fails_closed() {
    let fixture = Fixture::new("invalid-asp-policy");
    fixture.git(&["init", "--quiet"]);
    fixture.write("asp.toml", "[discovery]\nignoreDirs = [\"target\"]\n");
    fixture.git(&["add", "asp.toml"]);
    let error = discover_repository_candidate_snapshot(&fixture.root)
        .expect_err("legacy discovery keys must fail closed");
    let message = error.to_string();
    assert!(message.contains("ignoreDirs"), "{message}");
}
