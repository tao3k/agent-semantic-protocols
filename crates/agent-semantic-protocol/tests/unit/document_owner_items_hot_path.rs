use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time;

#[tokio::test]
async fn org_owner_items_stays_on_document_fast_path() {
    let repo_root = repo_root();
    let owner_path = "docs/10-19-rfcs/10.31-evidence-graph-codebase-memory-mcp-flow.org";
    assert!(
        repo_root.join(owner_path).is_file(),
        "expected fixture owner path to exist: {}",
        repo_root.join(owner_path).display()
    );

    let asp_bin = env!("CARGO_BIN_EXE_asp");
    let mut command = Command::new(asp_bin);
    command
        .current_dir(&repo_root)
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("org")
        .arg("search")
        .arg("owner")
        .arg(owner_path)
        .arg("items")
        .arg("--query")
        .arg("EvidenceGraph flow")
        .arg("--workspace")
        .arg(&repo_root)
        .arg("--view")
        .arg("seeds");

    let started = Instant::now();
    let output = run_with_timeout(command, Duration::from_millis(750)).await;
    let elapsed = started.elapsed();

    let output =
        output.unwrap_or_else(|error| panic!("{error}; asp_bin={asp_bin}; elapsed={elapsed:?}"));
    assert!(
        elapsed < Duration::from_millis(750),
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("protocol crate must live under crates/")
        .to_path_buf()
}

async fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    let child = command
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("failed to spawn asp: {error}"))?;

    match time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("failed to collect asp output: {error}")),
        Err(_) => Err(format!("asp command exceeded {timeout:?}")),
    }
}
