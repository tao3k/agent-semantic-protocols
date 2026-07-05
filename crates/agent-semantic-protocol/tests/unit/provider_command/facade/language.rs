use std::env;
use std::path::PathBuf;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_cache_source_fixture, write_echo_provider, write_pwd_provider,
};

#[test]
fn language_facade_discovers_activation_from_child_directory() {
    let root = temp_project_root("child-search-facade");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let child_dir = root.join("nested").join("workspace");
    std::fs::create_dir_all(&child_dir).expect("create child directory");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);
    let root_arg = root.to_str().expect("utf8 root").to_string();

    let output = asp_command(&root)
        .current_dir(&child_dir)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "agent",
            "doctor",
            "--workspace",
            root_arg.as_str(),
            "--json",
        ])
        .output()
        .expect("run asp rust agent doctor");

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
fn language_facade_version_does_not_require_activation() {
    let root = temp_project_root("language-version-without-activation");

    for arg in ["--version", "version"] {
        let output = asp_command(&root)
            .args(["rust", arg])
            .output()
            .expect("run asp rust version");

        assert!(
            output.status.success(),
            "arg={arg} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout"),
            format!("asp {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_unsupported_language_without_unrelated_provider_recovery() {
    let root = temp_project_root("language-unsupported-facade");
    write_activation(
        &root,
        &[
            provider("gerbil-scheme", Vec::new()),
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = asp_command(&root)
        .args([
            "scheme",
            "search",
            "lexical",
            "demo",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run unsupported language facade");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported ASP language facade `scheme`"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "Known language facades: rust|typescript|python|julia|gerbil-scheme|org|md."
        ),
        "{stderr}"
    );
    assert!(stderr.contains("asp providers"), "{stderr}");
    assert!(stderr.contains("asp fd -query"), "{stderr}");
    assert!(
        stderr.contains("Do not switch to an unrelated active facade"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("asp typescript search lexical"),
        "{stderr}"
    );
    assert!(!stderr.contains("Suggested matching facade"), "{stderr}");
    assert!(!stderr.contains("asp gerbil-scheme search"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_uses_manifest_child_as_provider_project_hint() {
    let root = temp_project_root("child-package-search-facade");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let child_dir = root.join("languages").join("rust-lang-project-harness");
    std::fs::create_dir_all(&child_dir).expect("create child dir");
    std::fs::write(
        child_dir.join("Cargo.toml"),
        "[package]\nname = \"nested-rust-lang-project-harness\"\nversion = \"0.1.0\"\n",
    )
    .expect("write child manifest");
    write_cache_source_fixture(&child_dir);
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .current_dir(&child_dir)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "query",
            "src/lib.rs",
            "--query",
            "demo",
            "--workspace",
            ".",
            "--code",
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
    let expected_cache_root = crate::provider_command::support::cache_root(&root);
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
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(
        &root,
        &[provider(
            "rust",
            vec![bin_dir.join("rs-harness").display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .current_dir(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--workspace",
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
    assert!(
        !argv
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|arg| arg == "."),
        "provider argv should not retain an already-selected project root: {argv:?}"
    );
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
    let cache_home = root.join(".cache");
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
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["typescript", "check", "--changed", "."])
        .output()
        .expect("run asp typescript check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "ts args=[check][--changed]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}
