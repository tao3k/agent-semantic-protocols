use std::{fs, path::PathBuf, process};

use crate::cli_args::parse_client_args;

#[test]
fn positional_paths_are_forwarded_without_project_root_inference() {
    let cwd = temp_dir("double-root");
    let provider_root = cwd.join("rust-provider");
    fs::create_dir_all(&provider_root).expect("create provider root");
    fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");

    let parsed = parse_client_args(
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
    .expect("positional paths are provider args");

    assert_eq!(parsed.project_root, cwd);
    assert_eq!(
        parsed.forwarded_args,
        vec![
            "ingest",
            "items",
            "tests",
            "--view",
            "seeds",
            provider_root.to_str().expect("utf8 provider root"),
            ".",
        ]
    );
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn selector_dot_does_not_count_as_extra_project_root() {
    let cwd = temp_dir("selector-dot");

    let parsed = parse_client_args(
        vec![
            "query".to_string(),
            "--workspace".to_string(),
            ".".to_string(),
            "--selector".to_string(),
            ".".to_string(),
            "--code".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("selector dot is an option value");

    assert_eq!(parsed.forwarded_args, vec!["--selector", ".", "--code"]);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn workspace_flag_selects_project_root_without_provider_forwarding() {
    let cwd = temp_dir("workspace-flag");
    let workspace = cwd.join("workspace");
    fs::create_dir_all(&workspace).expect("create workspace");

    let parsed = parse_client_args(
        vec![
            "query".to_string(),
            "--from-hook".to_string(),
            "direct-source-read".to_string(),
            "--workspace".to_string(),
            "workspace".to_string(),
            "--selector".to_string(),
            "crates/example/src/lib.rs:1:20".to_string(),
            "--source".to_string(),
            "worktree".to_string(),
            "--code".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("workspace flag selects the client project root");

    assert_eq!(
        parsed.project_root,
        std::fs::canonicalize(&workspace).expect("canonical workspace")
    );
    assert_eq!(
        parsed.forwarded_args,
        vec![
            "--from-hook",
            "direct-source-read",
            "--selector",
            "crates/example/src/lib.rs:1:20",
            "--source",
            "worktree",
            "--code",
        ]
    );
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn search_view_option_value_stays_provider_arg_without_project_root_inference() {
    let cwd = temp_dir("search-view-option-value");
    let seeds_root = cwd.join("seeds");
    fs::create_dir_all(&seeds_root).expect("create seeds root");
    fs::write(
        seeds_root.join("Cargo.toml"),
        "[package]\nname = \"seeds\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write seeds manifest");

    let parsed = parse_client_args(
        vec![
            "search".to_string(),
            "lexical".to_string(),
            "source_index_fixture".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            "--receipt-json".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("view option value is not a project root");

    assert_eq!(parsed.project_root, cwd);
    assert_eq!(
        parsed.forwarded_args,
        vec!["lexical", "source_index_fixture", "--view", "seeds"]
    );
    assert!(parsed.receipt_json);
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn workspace_flag_allows_explicit_project_root_outside_activation_root() {
    let cwd = temp_dir("workspace-flag-outside");
    let outside = temp_dir("workspace-flag-outside-target");

    let parsed = parse_client_args(
        vec![
            "query".to_string(),
            "--workspace".to_string(),
            outside.display().to_string(),
            "--selector".to_string(),
            "src/lib.rs:1:20".to_string(),
            "--code".to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("explicit workspace flag may target an external project root");

    assert_eq!(parsed.activation_root, cwd);
    assert_eq!(
        parsed.project_root,
        fs::canonicalize(&outside).expect("canonical outside root")
    );
    assert_eq!(
        parsed.forwarded_args,
        vec!["--selector", "src/lib.rs:1:20", "--code"]
    );
    let _ = fs::remove_dir_all(parsed.activation_root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn workspace_flag_after_default_query_does_not_leave_orphan_provider_arg() {
    let cwd = temp_dir("workspace-default-query");

    let parsed = parse_client_args(
        vec![
            "query".to_string(),
            "src/module.py".to_string(),
            "--term".to_string(),
            "parse_semantic_search_args".to_string(),
            "--workspace".to_string(),
            ".".to_string(),
        ],
        cwd.clone(),
        Some("python"),
    )
    .expect("workspace flag is consumed before positional root inference");

    assert_eq!(
        parsed.project_root,
        std::fs::canonicalize(&cwd).expect("canonical cwd")
    );
    assert_eq!(
        parsed.forwarded_args,
        vec!["src/module.py", "--term", "parse_semantic_search_args"]
    );
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn frontier_receipt_out_is_owned_by_client_runtime() {
    let cwd = temp_dir("frontier-receipt-out");
    let receipt_path = cwd.join("frontier-receipt.json");

    let parsed = parse_client_args(
        vec![
            "search".to_string(),
            "lexical".to_string(),
            "semantic-fact-frontier-receipt".to_string(),
            "owner".to_string(),
            "tests".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            "--frontier-receipt-out".to_string(),
            receipt_path.display().to_string(),
            "--workspace".to_string(),
            ".".to_string(),
        ],
        cwd.clone(),
        Some("python"),
    )
    .expect("frontier receipt path is a client runtime option");

    assert_eq!(parsed.frontier_receipt_out, Some(receipt_path));
    assert_eq!(
        parsed.forwarded_args,
        vec![
            "lexical",
            "semantic-fact-frontier-receipt",
            "owner",
            "tests",
            "--view",
            "seeds",
        ]
    );
    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn positional_marker_path_stays_provider_arg_without_workspace_flag() {
    let cwd = temp_dir("positional-root-activation");
    let provider_root = cwd.join("languages/rust-lang-project-harness");
    fs::create_dir_all(&provider_root).expect("create provider root");
    fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-lang-project-harness\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");

    let parsed = parse_client_args(
        vec![
            "search".to_string(),
            "workspace".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            provider_root.display().to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("positional marker path is a provider arg");

    assert_eq!(parsed.activation_root, cwd);
    assert_eq!(parsed.project_root, parsed.activation_root);
    assert_eq!(
        parsed.forwarded_args,
        vec![
            "workspace",
            "--view",
            "seeds",
            provider_root.to_str().expect("utf8 provider root")
        ]
    );
    let _ = fs::remove_dir_all(parsed.activation_root);
}

#[test]
fn external_positional_path_stays_provider_arg_without_workspace_flag() {
    let cwd = temp_dir("positional-root-outside");
    let outside = temp_dir("positional-root-outside-target");
    fs::write(
        outside.join("Cargo.toml"),
        "[package]\nname = \"outside-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");

    let parsed = parse_client_args(
        vec![
            "search".to_string(),
            "workspace".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            outside.display().to_string(),
        ],
        cwd.clone(),
        Some("rust"),
    )
    .expect("external positional path is a provider arg");

    assert_eq!(parsed.project_root, cwd);
    assert_eq!(
        parsed.forwarded_args,
        vec![
            "workspace",
            "--view",
            "seeds",
            outside.to_str().expect("utf8 outside root")
        ]
    );
    let _ = fs::remove_dir_all(cwd);
    let _ = fs::remove_dir_all(outside);
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("asp-cli-args-{}-{name}", process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
