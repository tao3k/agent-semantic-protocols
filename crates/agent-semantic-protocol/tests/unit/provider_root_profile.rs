use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_project(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temporary project");
    root
}

fn asp_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.current_dir(root);
    command
}

#[test]
fn root_search_infers_rust_from_profile_extension() {
    let root = temporary_project("root-profile-extension");
    let state_home = root.join(".state");
    std::fs::create_dir_all(&state_home).expect("create isolated state home");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write Rust fixture");

    let output = asp_command(&root)
        .env("ASP_STATE_HOME", state_home)
        .args([
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run root search with profile inference");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(!stderr.contains("requires --language"), "{stderr}");
    assert!(stderr.contains("endpoint"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_requires_language_without_profile_extension() {
    let root = temporary_project("root-profile-missing-extension");
    let output = asp_command(&root)
        .args([
            "search",
            "lexical",
            "source_index_fixture",
            "project_marker",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run root search without an inferable language");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(stderr.contains("requires --language"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}
