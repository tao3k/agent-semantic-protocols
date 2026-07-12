use std::path::Path;
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

    let output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "py-harness",
            "--term",
            "direct-source-read",
            "--term",
            "python adapter",
            "--workspace",
            ".",
            "--view",
            "metadata",
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
        stdout.contains(
            "|next search-lexical=\"asp md search lexical --query py-harness --query '<related-seed>' --workspace . --view seeds\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|next query-single-term=\"asp md query --term py-harness --workspace . --view metadata\""
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|next selector-source=\"rerun metadata query and use an emitted structuralSelector\""
        ),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn markdown_query_skips_hidden_directories_by_default() {
    let root = temp_project_root("md-query-hidden-dir");
    std::fs::write(root.join("README.md"), "# Project\n\nVisible prose.\n")
        .expect("write markdown fixture");
    let cache_dir = root.join(".cache");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    std::fs::write(
        cache_dir.join("generated.md"),
        "# Generated\n\ncached-secret-token\n",
    )
    .expect("write hidden markdown fixture");

    let output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "cached-secret-token",
            "--workspace",
            ".",
            "--view",
            "metadata",
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
        stdout.contains("[query] lang=md terms=1 root=. hit=0"),
        "{stdout}"
    );
    assert!(!stdout.contains(".cache/generated.md"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn markdown_query_can_include_configured_hidden_directories() {
    let root = temp_project_root("md-query-hidden-dir-config");
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    std::fs::write(
        &config_path,
        "[discovery]\nincludeHiddenDirNames = [\".cache\"]\n",
    )
    .expect("write asp config");
    std::fs::write(root.join("README.md"), "# Project\n\nVisible prose.\n")
        .expect("write markdown fixture");
    let cache_dir = root.join(".cache");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    std::fs::write(
        cache_dir.join("generated.md"),
        "# Generated\n\ncached-secret-token\n",
    )
    .expect("write hidden markdown fixture");

    let output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "cached-secret-token",
            "--workspace",
            ".",
            "--view",
            "metadata",
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
        stdout.contains("[query] lang=md terms=1 root=. hit=1"),
        "{stdout}"
    );
    assert!(stdout.contains(".cache/generated.md"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn markdown_provider_can_be_disabled_from_project_config() {
    let root = temp_project_root("md-disabled-config");
    std::fs::write(root.join("asp.toml"), "[providers.md]\nenabled = false\n")
        .expect("write asp config");
    std::fs::write(root.join("README.md"), "# Project\n\nVisible prose.\n")
        .expect("write markdown fixture");

    let output = asp_command(&root)
        .args([
            "md",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp md search");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("language `md` is disabled by asp.toml"),
        "{stderr}"
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

fn asp_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .current_dir(root)
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CODEX_PARENT_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID");
    command
}
