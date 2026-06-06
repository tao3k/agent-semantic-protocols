use std::{fs, path::PathBuf, process};

use crate::cli_args::parse_client_args;

#[test]
fn rejects_adjacent_positional_project_roots() {
    let cwd = temp_dir("double-root");
    let provider_root = cwd.join("rust-provider");
    fs::create_dir_all(&provider_root).expect("create provider root");
    fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");

    let err = parse_client_args(
        vec![
            "search".to_string(),
            "ingest".to_string(),
            "items".to_string(),
            "tests".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            provider_root.display().to_string(),
            ".".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect_err("double root should fail");

    assert_eq!(err, "expected at most one PROJECT_ROOT argument");
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn selector_dot_does_not_count_as_extra_project_root() {
    let cwd = temp_dir("selector-dot");

    let parsed = parse_client_args(
        vec![
            "query".to_string(),
            "--selector".to_string(),
            ".".to_string(),
            "--code".to_string(),
            ".".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("selector dot is an option value");

    assert_eq!(parsed.forwarded_args, vec!["--selector", ".", "--code"]);
    let _ = fs::remove_dir_all(cwd);
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("asp-cli-args-{}-{name}", process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
