use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
    write_pwd_provider,
};

#[test]
fn rust_search_facade_fans_out_multiple_trailing_scope_roots() {
    let root = temp_project_root("rust-search-facade-multi-scope");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("crates/agent-semantic-hook")).expect("create hook scope");
    std::fs::create_dir_all(root.join("crates/agent-semantic-protocol"))
        .expect("create protocol scope");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "fzf",
            "--query-set",
            "reasonKind",
            "--query-set",
            "RawBroadSearch",
            "owner",
            "tests",
            "--view",
            "seeds",
            "crates/agent-semantic-hook",
            "crates/agent-semantic-protocol",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        concat!(
            "rs args=[search][fzf][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-hook]\n",
            "rs args=[search][fzf][--query-set][reasonKind][--query-set][RawBroadSearch][owner][tests][--view][seeds][crates/agent-semantic-protocol]\n",
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_rejects_multiple_positional_project_roots() {
    let root = temp_project_root("rust-search-facade-double-root");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("rust-provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "ingest",
            "items",
            "tests",
            "--view",
            "seeds",
            "rust-provider",
            ".",
        ])
        .output()
        .expect("run asp rust search");

    assert!(!output.status.success(), "stdout={:?}", output.stdout);
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("expected at most one PROJECT_ROOT argument"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_strips_explicit_workspace_before_provider_backend() {
    let root = temp_project_root("rust-search-facade-explicit-workspace");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("rust-provider");
    std::fs::create_dir_all(&provider_root).expect("create provider root");
    std::fs::write(
        provider_root.join("Cargo.toml"),
        "[package]\nname = \"rust-provider\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "prime",
            "--workspace",
            "rust-provider",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with("[search-prime] root=rust-provider"),
        "{stdout}"
    );
    assert!(
        stdout.contains("alg=native-fd-prime-frontier-v1"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn check_facade_uses_positional_existing_directory_as_project_root() {
    let root = temp_project_root("check-facade-positional-directory-root");
    let bin_dir = root.join(".bin");
    let provider_root = root.join("fixture");
    std::fs::create_dir_all(provider_root.join("src")).expect("create provider root");
    write_pwd_provider(&bin_dir, "gerbil-scheme-harness");
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["gerbil-scheme", "check", "fixture"])
        .output()
        .expect("run asp gerbil-scheme check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!(
            "{}\n",
            std::fs::canonicalize(&provider_root)
                .expect("canonical provider root")
                .display()
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rust_search_facade_rejects_explicit_workspace_outside_activation_workspace() {
    let root = temp_project_root("rust-search-facade-workspace-boundary");
    let bin_dir = root.join(".bin");
    let outside_root = root.parent().expect("temp root parent").join(format!(
        "{}-outside",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("temp root name")
    ));
    std::fs::create_dir_all(&outside_root).expect("create outside root");
    std::fs::write(
        outside_root.join("Cargo.toml"),
        "[package]\nname = \"outside\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write outside manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "prime",
            "--workspace",
            outside_root.to_str().expect("outside root utf8"),
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search");

    assert!(!output.status.success(), "stdout={:?}", output.stdout);
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("is outside workspace"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside_root);
}

#[test]
fn rust_search_facade_rejects_positional_project_root_outside_activation_workspace() {
    let root = temp_project_root("rust-search-facade-positional-workspace-boundary");
    let bin_dir = root.join(".bin");
    let outside_root = root.parent().expect("temp root parent").join(format!(
        "{}-outside",
        root.file_name()
            .and_then(|name| name.to_str())
            .expect("temp root name")
    ));
    std::fs::create_dir_all(&outside_root).expect("create outside root");
    std::fs::write(
        outside_root.join("Cargo.toml"),
        "[package]\nname = \"outside\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write outside manifest");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "prime",
            "--view",
            "seeds",
            outside_root.to_str().expect("outside root utf8"),
        ])
        .output()
        .expect("run asp rust search");

    assert!(!output.status.success(), "stdout={:?}", output.stdout);
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("is outside workspace"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside_root);
}
