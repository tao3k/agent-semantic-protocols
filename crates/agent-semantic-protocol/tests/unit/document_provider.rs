use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn markdown_query_no_hit_returns_recovery_actions() {
    let root = temp_project_root("md-query-no-hit");
    std::fs::write(
        root.join("README.md"),
        "# Project\n\nOnly unrelated prose.\n",
    )
    .expect("write markdown fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .args([
            "md",
            "query",
            "--term",
            "py-harness",
            "--term",
            "direct-source-read",
            "--term",
            "python adapter",
            "--view",
            "metadata",
            ".",
        ])
        .output()
        .expect("run asp md query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("query stdout");
    assert!(
        stdout.contains("[query] lang=md terms=3 root=. hit=0"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|no-hit reason=empty-intersection combine=all-terms"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|next search-fzf=\"asp md search fzf py-harness --view seeds .\""),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|next query-single-term=\"asp md query --term py-harness --view metadata .\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|next direct-read-requires=\"asp md query --from-hook direct-source-read --selector <path:start-end> .\""
        ),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}
