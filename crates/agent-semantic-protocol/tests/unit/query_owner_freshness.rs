use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn exact_selector_code_reads_modified_source_without_stale_index() {
    let root = temp_project_root("exact-selector-freshness");
    fs::create_dir_all(root.join("src")).expect("create src");
    let owner = root.join("src/lib.rs");
    fs::write(&owner, "pub fn alpha() {\n    let value = 1;\n}\n").expect("write first source");

    let first = run_exact_selector_query(&root);
    assert!(first.contains("let value = 1;"), "{first}");

    fs::write(&owner, "pub fn alpha() {\n    let value = 2;\n}\n").expect("write second source");

    let second = run_exact_selector_query(&root);
    assert!(second.contains("let value = 2;"), "{second}");
    assert!(
        !second.contains("let value = 1;"),
        "exact selector query returned stale source after owner rewrite: {second}"
    );

    let _ = fs::remove_dir_all(root);
}

fn run_exact_selector_query(root: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(root)
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .arg("rust")
        .arg("query")
        .arg("--selector")
        .arg("rust://src/lib.rs#item/function/alpha")
        .arg("--workspace")
        .arg(root)
        .arg("--code")
        .output()
        .expect("run asp query");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("query stdout")
}

fn temp_project_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{name}-{nonce}"))
}
