use crate::provider_command::support::{
    asp_command, prepend_path, provider, provider_with_owner_items, temp_project_root,
    write_activation, write_marker_provider, write_provider_bin_config,
};

fn refresh_source_index(root: &std::path::Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "rebuild"])
        .output()
        .expect("run asp cache source-index rebuild");
    assert!(
        output.status.success(),
        "source-index rebuild failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn search_pipe_auto_defers_source_index_for_multi_clause_query() {
    let root = temp_project_root("search-pipe-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"search-pipe-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "source_index_fixture|src/lib.rs",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-pipe]"), "{stdout}");
    assert!(stdout.contains("source=source-index"), "{stdout}");
    assert!(
        stdout.contains("sourceTrace=sourceIndex:deferred"),
        "{stdout}"
    );
    assert!(stdout.contains("search-overlay:skipped"), "{stdout}");
    assert!(
        stdout.contains("ownerCoverage=bestOwner=src/lib.rs"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("nextCommand=asp fd -query 'source_index_fixture|src/lib.rs' --workspace ."),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "search-overlay fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_owner_items_source_index_trace_includes_search_frame_receipt() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_project_root("search-owner-items-source-index-frame");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"search-owner-items-source-index-frame\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    std::fs::create_dir_all(&bin_dir).expect("create provider bin");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{marker}'\nprintf '[search-owner] q=src/lib.rs pkg=. selector=items alg=source-index-owner-items\\n'\nprintf 'O=owner:path(src/lib.rs)!owner;I=item:symbol(source_index_fixture)@src/lib.rs:1:1!syntax\\n'\n",
            marker = marker.display()
        ),
    )
    .expect("write owner-items provider");
    let mut permissions = std::fs::metadata(&provider_path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&provider_path, permissions).expect("chmod provider");
    write_provider_bin_config(&root, "rust", &provider_path);
    write_activation(&root, &[provider_with_owner_items("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "source_index_fixture",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search owner items");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.contains("|sourceIndex"), "{stdout}");
    assert!(!stdout.contains("sourceTrace=\"source-index"), "{stdout}");
    assert!(
        stdout.contains("item:symbol(source_index_fixture)"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "dynamic owner-items should not route source-index trace into the provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_skips_source_index_for_generic_action_query() {
    let root = temp_project_root("search-pipe-source-index-query-gate");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"search-pipe-source-index-query-gate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "owner-items|selector-code",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-pipe]"), "{stdout}");
    assert!(stdout.contains("source=search-overlay"), "{stdout}");
    assert!(stdout.contains("sourceIndex:query-gate"), "{stdout}");
    assert!(stdout.contains("search-overlay:empty"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
