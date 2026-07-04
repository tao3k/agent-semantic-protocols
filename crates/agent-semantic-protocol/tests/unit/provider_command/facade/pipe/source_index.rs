use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

fn refresh_source_index(root: &std::path::Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "refresh"])
        .output()
        .expect("run asp cache source-index refresh");
    assert!(
        output.status.success(),
        "source-index refresh failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn search_pipe_auto_uses_db_engine_source_index_before_search_overlay() {
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
            "source_index_fixture",
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
    assert!(stdout.contains("sourceTrace=sourceIndex:used"), "{stdout}");
    assert!(stdout.contains("search-overlay:skipped"), "{stdout}");
    assert!(
        stdout.contains("ownerCoverage=bestOwner=src/lib.rs"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp rust search owner src/lib.rs items --query source_index_fixture --workspace . --view seeds"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("O=owner:path(src/lib.rs)"), "{stdout}");
    assert!(
        !marker.exists(),
        "source-index fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_owner_items_source_index_trace_includes_search_frame_receipt() {
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
    assert!(stdout.contains("|sourceIndex status=hit"), "{stdout}");
    assert!(stdout.contains("nextCommand="), "{stdout}");
    assert!(
        stdout.contains("recommendedNext=search-owner-items"),
        "{stdout}"
    );
    assert!(
        stdout.contains("actionFrontier=search-owner-items,query-exact-selector"),
        "{stdout}"
    );
    assert!(
        stdout.contains("sourceTrace=\"source-index:src/lib.rs\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=inline-code-in-search,raw-read,repeat-owner"),
        "{stdout}"
    );
    assert!(
        stdout.contains("whereFrame=\"owner:src/lib.rs\""),
        "{stdout}"
    );
    assert!(stdout.contains("howFrame=owner-items-search"), "{stdout}");
    assert!(
        !marker.exists(),
        "source-index fast path should not spawn provider"
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
            "owner-items selector-code",
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
    assert!(
        stdout.contains("sourceTrace=sourceIndex:skipped"),
        "{stdout}"
    );
    assert!(stdout.contains("reason=query-gate"), "{stdout}");
    assert!(!stdout.contains("sourceIndex:used"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
