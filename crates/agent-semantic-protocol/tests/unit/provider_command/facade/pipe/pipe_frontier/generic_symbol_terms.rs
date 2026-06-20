use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn search_pipe_keeps_query_symbols_without_fixed_generic_wordlist() {
    let root = temp_project_root("search-pipe-no-fixed-generic-symbol-list");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/http")).expect("create rust src");
    std::fs::write(root.join("src/http/client.rs"), "pub struct ApiClient;\n")
        .expect("write client source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "API ApiClient owner frontier",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe generic symbol term");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("queryTerms=API:symbol,ApiClient:symbol,owner:context,frontier:context"),
        "{stdout}"
    );
    assert!(
        stdout.contains("strongCoverage=matched=ApiClient weak=API"),
        "{stdout}"
    );
    assert!(stdout.contains("fdQuery=API|ApiClient"), "{stdout}");
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_wide_owner_axis_query_materializes_fd_without_protocol_terms() {
    let root = temp_project_root("search-pipe-wide-owner-axis-fd");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/project_runtime")).expect("create rust src");
    std::fs::write(
        root.join("src/project_runtime/session_content.rs"),
        "pub struct SessionContent;\n",
    )
    .expect("write session content source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "wide query rg drift fd owner recovery project runtime session content source anchor graph turbo handoff",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe wide owner axis query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("risk=package-drift"), "{stdout}");
    assert!(
        stdout.contains("fdQuery=recovery|project|runtime|session|content|source|anchor"),
        "{stdout}"
    );
    assert!(
        stdout.contains("A1=fd-query(query=recovery|project|runtime|session|content|source|anchor"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("fdQuery=query|rg|fd|owner|graph|turbo"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_scope_query_does_not_spawn_provider_facts() {
    let root = temp_project_root("search-pipe-scope-no-provider-facts");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create rust src");
    std::fs::write(root.join("src/scope_gate.rs"), "pub fn scope_gate() {}\n")
        .expect("write scope gate source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "scope gate cache workflow",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe scope query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-pipe]"), "{stdout}");
    assert!(stdout.contains("providerFacts:skipped["), "{stdout}");
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_owner_drift_query_pack_precedes_fd_preview() {
    let root = temp_project_root("search-pipe-owner-drift-fd-first");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in [
        "crates/alpha/src",
        "crates/beta/src",
        "crates/gamma/src",
        "crates/delta/src",
    ] {
        std::fs::create_dir_all(root.join(path)).expect("create rust package src");
    }
    std::fs::write(
        root.join("crates/alpha/src/candidate_ranking.rs"),
        "pub fn candidate_ranking() {}\n",
    )
    .expect("write candidate ranking source");
    std::fs::write(
        root.join("crates/beta/src/parser_finder.rs"),
        "pub fn parser_finder() {}\n",
    )
    .expect("write parser finder source");
    std::fs::write(
        root.join("crates/gamma/src/local_item_evidence.rs"),
        "pub fn local_item_evidence() {}\n",
    )
    .expect("write local item evidence source");
    std::fs::write(
        root.join("crates/delta/src/recovery_density.rs"),
        "pub fn fd_recovery_density() {}\n",
    )
    .expect("write recovery density source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "graph turbo owner candidate ranking weak local item evidence parser finder hit density fd recovery",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe owner drift fd first");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(stdout.contains("fdPreview=ownerCandidates="), "{stdout}");
    assert!(stdout.contains("A1=rg-query-set("), "{stdout}");
    assert!(!stdout.contains("A1=owner-items("), "{stdout}");
    assert!(stdout.contains("A2=fd-query("), "{stdout}");
    assert!(
        stdout.contains("recommendedNext=A1.rg-query-set"),
        "{stdout}"
    );
    assert!(stdout.contains("nextCommand=asp rg -query"), "{stdout}");
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_owner_items_query_prefers_local_evidence_before_path_axes() {
    let root = temp_project_root("search-pipe-owner-items-local-evidence-first");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create rust src");
    std::fs::write(
        root.join("src/search_pipe_graph_turbo_owner_rank.rs"),
        [
            "pub fn ranked_candidate_paths() {}\n",
            "pub fn parser_finder_local_item_evidence_density() {}\n",
        ]
        .concat(),
    )
    .expect("write owner rank source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "graph turbo owner candidate ranking weak local item evidence parser finder hit density",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe owner-items local evidence first");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("ownerItems=src/search_pipe_graph_turbo_owner_rank.rs:ranked|candidate"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("ownerItems=src/search_pipe_graph_turbo_owner_rank.rs:graph|turbo"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
