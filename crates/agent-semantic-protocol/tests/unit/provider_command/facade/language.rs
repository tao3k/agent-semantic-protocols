use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider, write_pwd_provider,
};

#[test]
fn rust_search_facade_execs_activated_provider() {
    let root = temp_project_root("language-search-facade-cache");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        ("rust", "rs-harness", "rs"),
        ("typescript", "ts-harness", "ts"),
        ("python", "py-harness", "py"),
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    use std::os::unix::fs::PermissionsExt;
    for (_, binary, label) in providers.iter().copied() {
        let call_count_path = root.join(format!("{label}-provider-count"));
        let provider_path = bin_dir.join(binary);
        let script = format!(
            "#!/bin/sh\ncount=\"{}\"\ncurrent=0\nif [ -f \"$count\" ]; then current=$(cat \"$count\"); fi\ncurrent=$((current + 1))\nprintf '%s' \"$current\" > \"$count\"\nprintf '{} search prime --view seeds .\\n'\n",
            call_count_path.display(),
            label
        );
        std::fs::write(&provider_path, script).expect("write provider");
        let mut permissions = std::fs::metadata(&provider_path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&provider_path, permissions).expect("provider permissions");
    }
    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
            provider("python", Vec::new()),
        ],
    );
    for (language, _, label) in providers.iter().copied() {
        let call_count_path = root.join(format!("{label}-provider-count"));
        let expected_stdout = format!("{label} search prime --view seeds .\n");
        let first_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("PATH", &bin_dir)
            .env("PRJ_CACHE_HOME", &cache_home)
            .args([language, "search", "prime", "--view", "seeds", "."])
            .output()
            .expect("run facade first");
        assert!(
            first_output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&first_output.stderr)
        );
        assert_eq!(
            String::from_utf8(first_output.stdout).expect("stdout"),
            expected_stdout
        );
        assert_eq!(
            std::fs::read_to_string(&call_count_path).expect("read count"),
            "1"
        );
        let second_output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .env("PATH", &bin_dir)
            .env("PRJ_CACHE_HOME", &cache_home)
            .args([language, "search", "prime", "--view", "seeds", "."])
            .output()
            .expect("run facade second");
        assert!(
            second_output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&second_output.stderr)
        );
        assert_eq!(
            String::from_utf8(second_output.stdout).expect("stdout"),
            expected_stdout
        );
        assert_eq!(
            std::fs::read_to_string(&call_count_path).expect("read count"),
            "1"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_discovers_activation_from_child_directory() {
    let root = temp_project_root("child-search-facade");
    let bin_dir = root.join(".bin");
    let child_dir = root.join("nested").join("workspace");
    std::fs::create_dir_all(&child_dir).expect("create child directory");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .current_dir(&child_dir)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "search", "prime", "."])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual_root = PathBuf::from(String::from_utf8(output.stdout).expect("stdout").trim());
    assert_eq!(
        std::fs::canonicalize(actual_root).expect("canonical actual root"),
        std::fs::canonicalize(&root).expect("canonical expected root")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_uses_nested_package_root_when_child_has_manifest() {
    let root = temp_project_root("child-package-search-facade");
    let bin_dir = root.join(".bin");
    let child_dir = root.join("languages").join("rust-lang-project-harness");
    std::fs::create_dir_all(&child_dir).expect("create child dir");
    std::fs::write(
        child_dir.join("Cargo.toml"),
        "[package]\nname = \"nested-rust-lang-project-harness\"\nversion = \"0.1.0\"\n",
    )
    .expect("write child manifest");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .current_dir(&child_dir)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--query",
            "demo",
            "--code",
            ".",
            "--receipt-json",
        ])
        .output()
        .expect("run query facade");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let actual_root = PathBuf::from(stdout.trim());
    assert_eq!(
        std::fs::canonicalize(actual_root).expect("canonical actual root"),
        std::fs::canonicalize(&child_dir).expect("canonical child root")
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stderr).expect("receipt JSON");
    let expected_cache_root = root.join(".cache/agent-semantic-protocol/client");
    std::fs::create_dir_all(&expected_cache_root).expect("create expected cache root");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(
            receipt["cacheRoot"].as_str().expect("cacheRoot")
        ))
        .expect("canonical cache root"),
        std::fs::canonicalize(expected_cache_root).expect("canonical expected cache root")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_normalizes_relative_nested_project_root_arg() {
    let root = temp_project_root("relative-child-package-search-facade");
    let child_dir = root.join("languages").join("rust-lang-project-harness");
    std::fs::create_dir_all(&child_dir).expect("create child dir");
    std::fs::write(
        child_dir.join("Cargo.toml"),
        "[package]\nname = \"nested-rust-lang-project-harness\"\nversion = \"0.1.0\"\n",
    )
    .expect("write child manifest");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .current_dir(&root)
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "languages/rust-lang-project-harness",
            "--receipt-json",
        ])
        .output()
        .expect("run query facade");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let receipt: serde_json::Value = serde_json::from_slice(&output.stderr).expect("receipt JSON");
    let argv = receipt["providerCommands"][0]["argv"]
        .as_array()
        .expect("provider argv");
    assert_eq!(argv.last().and_then(serde_json::Value::as_str), Some("."));
    assert!(
        !argv
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|arg| arg == "languages/rust-lang-project-harness"),
        "provider argv should not keep a stale relative project root: {argv:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_selects_matching_provider_from_activation() {
    let root = temp_project_root("typescript-search-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_echo_provider(&bin_dir, "ts-harness", "ts");
    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["typescript", "search", "fzf", "parseSearchArgs", "."])
        .output()
        .expect("run asp typescript search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "ts args=[search][fzf][parseSearchArgs][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
