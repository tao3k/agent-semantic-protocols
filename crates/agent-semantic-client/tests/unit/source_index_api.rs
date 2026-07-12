use super::source_index_tracked_worktree_dirty_paths;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn dirty_paths_are_relative_to_the_index_root() {
    let root = std::env::temp_dir().join(format!(
        "asp-source-index-dirty-paths-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let index_root = root.join("workspace");
    std::fs::create_dir_all(index_root.join("src")).expect("create index source dir");
    std::fs::write(index_root.join("src/lib.rs"), "pub fn initial() {}\n")
        .expect("write index source");
    std::fs::write(root.join("README.md"), "initial\n").expect("write sibling file");
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "."]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    std::fs::write(index_root.join("src/lib.rs"), "pub fn changed() {}\n")
        .expect("dirty indexed source");
    std::fs::write(root.join("README.md"), "changed\n").expect("dirty sibling file");

    let dirty_paths = source_index_tracked_worktree_dirty_paths(&index_root)
        .expect("read tracked dirty paths for nested index root");

    assert_eq!(dirty_paths.len(), 1);
    assert!(dirty_paths.contains(Path::new("src/lib.rs")));
    let _ = std::fs::remove_dir_all(root);
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git command failed");
}
