use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[test]
fn org_owner_items_stays_on_document_fast_path() {
    let repo_root = repo_root();
    let owner_path = "docs/10-19-rfcs/10.31-evidence-graph-codebase-memory-mcp-flow.org";
    assert!(
        repo_root.join(owner_path).is_file(),
        "expected fixture owner path to exist: {}",
        repo_root.join(owner_path).display()
    );

    let asp_bin = env!("CARGO_BIN_EXE_asp");
    let warmup = org_owner_items_command(asp_bin, &repo_root, owner_path)
        .output()
        .unwrap_or_else(|error| panic!("failed to warm asp: {error}; asp_bin={asp_bin}"));
    assert!(
        warmup.status.success(),
        "org owner-items warmup should succeed; stderr={}",
        String::from_utf8_lossy(&warmup.stderr)
    );
    let mut command = org_owner_items_command(asp_bin, &repo_root, owner_path);

    let started = Instant::now();
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run asp: {error}; asp_bin={asp_bin}"));
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1500),
        "org owner-items should stay on the document fast path; elapsed={elapsed:?}; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "org owner-items document fast path should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[search-owner]")
            && stdout.contains("selector=items")
            && stdout.contains("alg=asp-dynamic-owner-items-v1"),
        "org owner-items should render a search-owner packet; stdout={stdout}"
    );
    assert!(
        stdout.contains("kind=heading") && stdout.contains("#item/heading/"),
        "org owner-items should expose heading items; stdout={stdout}"
    );
    assert!(
        stdout.contains("EvidenceGraph flow"),
        "org owner-items should match the RFC heading query; stdout={stdout}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("failed to execute provider"),
        "org owner-items must not recurse through provider execution; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn org_owner_items_command(
    asp_bin: &str,
    repo_root: &Path,
    owner_path: &str,
) -> std::process::Command {
    let mut command = std::process::Command::new(asp_bin);
    command
        .current_dir(repo_root)
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .arg("org")
        .arg("search")
        .arg("owner")
        .arg(owner_path)
        .arg("items")
        .arg("--query")
        .arg("Evidence Graph GraphRoute compact ranked evidence subagent receipt")
        .arg("--workspace")
        .arg(repo_root)
        .arg("--view")
        .arg("seeds");
    command
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("protocol crate must live under crates/")
        .to_path_buf()
}
