use crate::provider_command::support::{
    asp_command, install_state_home_provider, prepend_path, provider, provider_with_owner_items,
    temp_project_root, write_activation, write_marker_provider,
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
        .env(
            "ASP_WORKSPACE_RESIDENT_SERVICE_READINESS_TIMEOUT_MS",
            "5000",
        )
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
fn search_owner_items_uses_registered_native_owner_surface() {
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
            "#!/bin/sh\n\
printf called > '{marker}'\n\
request=$(cat)\n\
digest=$(printf '%s' \"$request\" | sed -n 's/.*\"contentDigest\":\"\\([0-9a-f]*\\)\".*/\\1/p')\n\
printf '%s\\n' \"{{\\\"schemaId\\\":\\\"agent.semantic-protocols.provider-native-owner-search-response\\\",\\\"schemaVersion\\\":\\\"1\\\",\\\"languageId\\\":\\\"rust\\\",\\\"providerId\\\":\\\"rs-harness\\\",\\\"requestedOwnerPath\\\":\\\"src/lib.rs\\\",\\\"requestedProjectionMode\\\":\\\"items\\\",\\\"sourceContentDigest\\\":\\\"$digest\\\",\\\"parsedOwnerCount\\\":1,\\\"projectionCompleteness\\\":\\\"complete-owner\\\",\\\"projections\\\":[{{\\\"structuralSelector\\\":\\\"rust://src/lib.rs#item/function/source_index_fixture\\\",\\\"itemKind\\\":\\\"function\\\",\\\"itemName\\\":\\\"source_index_fixture\\\",\\\"captureName\\\":\\\"function.name\\\",\\\"signature\\\":\\\"pub fn source_index_fixture()\\\",\\\"sourceByteStart\\\":0,\\\"sourceByteEnd\\\":32}}]}}\"\n",
            marker = marker.display()
        ),
    )
    .expect("write owner-items provider");
    let mut permissions = std::fs::metadata(&provider_path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&provider_path, permissions).expect("chmod provider");
    install_state_home_provider(&root, "rust", &provider_path);
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
        marker.exists(),
        "registered native owner-items must execute through the provider-owned surface"
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
