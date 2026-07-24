use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn search_pipe_help_does_not_require_activation_or_provider_spawn() {
    let root = temp_project_root("search-pipe-help");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "search", "pipe", "--help"])
        .output()
        .expect("run asp rust search pipe help");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("usage: asp rust search pipe <refinement-query>"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refinement frontier after lexical/dependency evidence is ambiguous"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("Do not use pipe for CLI-command lexical searches"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("natural-intent"), "stdout={stdout}");
    assert!(stdout.contains("--workspace PROJECT_ROOT"));
    assert!(stdout.contains("--selector SELECTOR"));
    assert!(stdout.contains("--query TERMS"));
    assert!(stdout.contains("--source auto|provider|search-overlay|ingest"));
    assert!(output.stderr.is_empty());
    assert!(!marker.exists(), "help should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_version_does_not_require_activation_or_provider_spawn() {
    let root = temp_project_root("search-pipe-version");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "search", "pipe", "--version"])
        .output()
        .expect("run asp rust search pipe version");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        format!("asp {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
    assert!(!marker.exists(), "version should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_selector_seed_renders_single_command_frontier_without_provider_spawn() {
    let root = temp_project_root("search-pipe-selector-seed");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let selector = "rust://crates/agent-semantic-protocol/src/command/provider_process.rs#item/fn/provider_invocation_with_profile";
    let query = "runtime_profile_invocation RuntimeProfiles provider_command_prefix";
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "--selector",
            selector,
            "--query",
            query,
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe with selector seed");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("source=selector"), "{stdout}");
    assert!(
        stdout.contains(&format!("selectorSeed={selector}")),
        "{stdout}"
    );
    assert!(
        stdout.contains("ownerSeed=crates/agent-semantic-protocol/src/command/provider_process.rs"),
        "{stdout}"
    );
    assert!(
        stdout.contains("symbolSeed=provider_invocation_with_profile"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "nextCommand=asp rust query --selector '{selector}' --workspace . --code"
        )),
        "{stdout}"
    );
    assert!(!stdout.contains("commandHandles="), "{stdout}");
    assert!(
        stdout.contains("actionFrontier=A1.query-code,A2.owner-items,A3.rg-query"),
        "{stdout}"
    );
    assert!(stdout.contains("recommendedNext=A1.query-code"), "{stdout}");
    assert!(!stdout.contains("&&"), "{stdout}");
    assert!(
        !marker.exists(),
        "selector-seeded pipe should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_single_atom_query_does_not_render_pipe_plan_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-single-atom");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Connected",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe with a single seed atom");

    assert!(
        !output.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search pipe requires at least two query clauses"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "single-atom pipe should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_file_range_selector_seed_does_not_materialize_query_code() {
    let root = temp_project_root("search-pipe-file-range-selector-seed");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    let language_cases = [
        ("rust", "src/lib.rs:1:5"),
        ("typescript", "src/index.ts:1:5"),
        ("python", "src/main.py:1:5"),
        ("julia", "src/Main.jl:1:5"),
        ("gerbil-scheme", "src/main.ss:1:5"),
        ("org", "docs/index.org:1:5"),
        ("md", "README.md:1:5"),
    ];
    let providers = language_cases
        .iter()
        .map(|(language_id, _)| provider(*language_id, Vec::new()))
        .collect::<Vec<_>>();
    write_activation(&root, &providers);

    let query = "runtime_profile_invocation RuntimeProfiles provider_command_prefix";
    for (language_id, selector) in language_cases {
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language_id,
                "search",
                "pipe",
                "--selector",
                selector,
                "--query",
                query,
                "--workspace",
                ".",
                "--view",
                "seeds",
            ])
            .output()
            .expect("run asp search pipe with file-range selector seed");

        assert!(
            output.status.success(),
            "language={language_id} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.contains("source=selector"), "{stdout}");
        assert!(
            stdout.contains(&format!("selectorSeed={selector}")),
            "{stdout}"
        );
        assert!(!stdout.contains("query-code"), "{stdout}");
        assert!(
            !stdout.contains(&format!(
                "nextCommand=asp {language_id} query --selector {selector}"
            )),
            "{stdout}"
        );
        assert!(
            !stdout.contains(&format!(
                "nextCommand=asp {language_id} query --selector '{selector}'"
            )),
            "{stdout}"
        );
        assert!(
            stdout.contains("actionFrontier=A1.owner-items,A2.rg-query"),
            "{stdout}"
        );
        assert!(
            stdout.contains("recommendedNext=A1.owner-items"),
            "{stdout}"
        );
    }
    assert!(
        !marker.exists(),
        "file-range selector-seeded pipe should not spawn providers"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_stale_structural_selector_fails_instead_of_empty_success() {
    let root = temp_project_root("query-stale-structural-selector");
    std::fs::create_dir_all(root.join("src")).expect("create source root");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn collect_search_pipe_auto_acquisition() {}\n",
    )
    .expect("write current source");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://crates/agent-semantic-client/src/search_pipe_source.rs#item/function/collect_search_pipe_auto_acquisition",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run stale structural selector query");

    assert!(
        !output.status.success(),
        "stale structural selector must not exit successfully"
    );
    assert!(
        output.stdout.is_empty(),
        "stale structural selector must not emit empty-success evidence"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("stale-index exact selector resolved no code payload"),
        "{stderr}"
    );
    assert!(
        stderr.contains("crates/agent-semantic-client/src/search_pipe_source.rs"),
        "{stderr}"
    );
    assert!(
        stderr.contains("recommendedNext=asp rust search owner"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_rejects_removed_pipeline_option_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-pipe-option");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    let removed_option = format!("--{}", "pipe");

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(vec![
            "rust".to_string(),
            "search".to_string(),
            "pipe".to_string(),
            "HookDecision".to_string(),
            removed_option.clone(),
            "items,tests".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ])
        .output()
        .expect("run asp rust search pipe with removed pipeline option");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains(&format!("unknown search pipe option: {removed_option}")),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "removed pipeline option should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_rejects_json_alias_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-json-alias");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision",
            "--json",
            "--workspace",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe with removed json alias");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unknown search pipe option: --json"),
        "{stderr}"
    );
    assert!(!marker.exists(), "json alias should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_rejects_unknown_surface_without_provider_spawn() {
    let root = temp_project_root("search-pipe-rejects-unknown-surface");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "HookDecision",
            "--surface",
            "owner,commands",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe with unknown surface");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unknown search pipe option: --surface"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "unknown surface should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_package_option_scopes_search_overlay_frontier_without_provider_spawn() {
    let root = temp_project_root("search-pipe-package-option");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);
    std::fs::create_dir_all(root.join("src/compiler")).expect("create scoped source");
    std::fs::create_dir_all(root.join("src/server")).expect("create out-of-scope source");
    std::fs::write(
        root.join("src/compiler/program.ts"),
        "export function createProgram() { return 1; }\n",
    )
    .expect("write scoped source");
    std::fs::write(
        root.join("src/compiler/config.ts"),
        "export function createProgramConfig() { return 3; }\n",
    )
    .expect("write second scoped source");
    std::fs::write(
        root.join("src/server/program.ts"),
        "export function createProgramServer() { return 2; }\n",
    )
    .expect("write out-of-scope source");

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "createProgram|createProgramConfig",
            "--package",
            "src/compiler",
            "--source",
            "search-overlay",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe with package scope");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("source=search-overlay"), "{stdout}");
    assert!(
        stdout.contains("sourceSnapshot=schemaId=asp.source-snapshot.v1")
            && stdout.contains("sourceKind=editor-buffer")
            && stdout.contains("rootDigest=")
            && stdout.contains("baseRootDigest=")
            && stdout.contains("providerDigest=")
            && stdout.contains("dirtyPathsDigest="),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp typescript search owner")
            && stdout.contains("--workspace src/compiler --view seeds"),
        "{stdout}"
    );
    assert!(!stdout.contains("nextCommand=asp fd"), "{stdout}");
    assert!(!stdout.contains("src/server/program.ts"), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    assert!(
        !stdout.contains("query-code(selector=src/compiler/program.ts:"),
        "{stdout}"
    );
    assert!(!stdout.contains("owner:path(program.ts)"), "{stdout}");
    assert!(
        !marker.exists(),
        "finder-scoped pipe should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
