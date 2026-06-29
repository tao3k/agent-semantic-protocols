use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_check_failure_provider, write_marker_provider,
};

#[test]
fn reasoning_owner_query_is_asp_owned_and_does_not_spawn_provider() {
    let root = temp_project_root("search-reasoning-owner-query-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "fn unrelated() {}\nfn render_fast_prime_search() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "reasoning",
            "owner-query",
            "--owner",
            "src/lib.rs",
            "--query",
            "render_fast_prime_search",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search reasoning owner-query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-owner]"), "{stdout}");
    assert!(
        stdout.contains(
            "I=item:symbol(render_fast_prime_search)@rust://src/lib.rs#item/fn/render_fast_prime_search!syntax"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("syntax I selector=rust://src/lib.rs#item/fn/render_fast_prime_search displayLineRange=2:3 sourceLocatorHint=src/lib.rs:2:3 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"render_fast_prime_search\"))'"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(render_fast_prime_search)@src/lib.rs:"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("syntax I selector=src/lib.rs:2:3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("entries=owner-query(O,Q=>items+tests+dependency-usage)\n"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("owner-tests("),
        "owner-query reasoning should not infer owner-tests entry: {stdout}"
    );
    assert!(
        !marker.exists(),
        "owner-query fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scoped_owner_query_code_locator_replays_from_workspace_root() {
    let root = temp_project_root("search-reasoning-scoped-root-code-replay");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("crates/demo/src")).expect("create scoped src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/demo\"]\n",
    )
    .expect("write workspace manifest");
    std::fs::write(
        root.join("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write demo manifest");
    std::fs::write(
        root.join("crates/demo/src/lib.rs"),
        "fn cache_root() {\n    let value = 1;\n    let _ = value;\n}\n",
    )
    .expect("write scoped source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "reasoning",
            "owner-query",
            "--owner",
            "src/lib.rs",
            "--query",
            "cache_root",
            "--view",
            "seeds",
            "crates/demo",
        ])
        .output()
        .expect("run scoped asp rust search reasoning owner-query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains(
            "I=item:symbol(cache_root)@rust://crates/demo/src/lib.rs#item/fn/cache_root!syntax"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("syntax I selector=rust://crates/demo/src/lib.rs#item/fn/cache_root displayLineRange=1:4 sourceLocatorHint=crates/demo/src/lib.rs:1:4 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"cache_root\"))'"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(cache_root)@crates/demo/src/lib.rs:"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("syntax I selector=crates/demo/src/lib.rs:1:4"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "scoped owner-query fast path should not spawn provider"
    );

    let replay = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--from-hook",
            "item-skeleton",
            "--selector",
            "rust://crates/demo/src/lib.rs#item/fn/cache_root",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("replay scoped code locator");

    assert!(
        replay.status.success(),
        "status={:?} stdout={} stderr={}",
        replay.status.code(),
        String::from_utf8_lossy(&replay.stdout),
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay_stdout = String::from_utf8(replay.stdout).expect("replay stdout");
    assert!(
        replay_stdout.contains("fn cache_root() {"),
        "{replay_stdout}"
    );
    assert!(replay_stdout.contains("let value = 1;"), "{replay_stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_tests_and_owner_items_query_are_asp_owned() {
    let root = temp_project_root("search-owner-fast-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "fn unrelated() {}\nfn render_fast_prime_search() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let owner_tests = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "reasoning",
            "owner-tests",
            "--owner",
            "src/lib.rs",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search reasoning owner-tests");
    assert!(
        owner_tests.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&owner_tests.stderr)
    );
    let owner_tests_stdout = String::from_utf8(owner_tests.stdout).expect("stdout");
    assert!(
        owner_tests_stdout.contains("T=test:path(src/lib.rs)!tests"),
        "{owner_tests_stdout}"
    );

    let owner_items = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--query",
            "render_fast_prime_search",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search owner items");
    assert!(
        owner_items.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&owner_items.stderr)
    );
    let owner_items_stdout = String::from_utf8(owner_items.stdout).expect("stdout");
    assert!(
        owner_items_stdout.contains(
            "I=item:symbol(render_fast_prime_search)@rust://src/lib.rs#item/fn/render_fast_prime_search!syntax"
        ),
        "{owner_items_stdout}"
    );
    assert!(
        owner_items_stdout.contains("syntax I selector=rust://src/lib.rs#item/fn/render_fast_prime_search displayLineRange=2:3 sourceLocatorHint=src/lib.rs:2:3 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"render_fast_prime_search\"))'"),
        "{owner_items_stdout}"
    );
    assert!(
        !owner_items_stdout.contains("I=item:symbol(render_fast_prime_search)@src/lib.rs:"),
        "{owner_items_stdout}"
    );
    assert!(
        !owner_items_stdout.contains("syntax I selector=src/lib.rs:2:3"),
        "{owner_items_stdout}"
    );
    assert!(
        owner_items_stdout.contains("entries=owner-query(O,Q=>items+tests+dependency-usage)"),
        "{owner_items_stdout}"
    );
    assert!(
        !owner_items_stdout.contains("frontier="),
        "{owner_items_stdout}"
    );
    assert!(
        !marker.exists(),
        "owner fast paths should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_failure_frontier_is_asp_owned_and_points_to_hot_blocks() {
    let root = temp_project_root("search-failure-frontier-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create source");
    std::fs::write(
        root.join("src/cache_cli/writeback.rs"),
        "fn write_prompt_output_artifact() {\n    let request_fingerprint = \"prompt-output\";\n    let file_hash = request_fingerprint;\n    let _ = file_hash;\n}\n\nfn load_prompt_output_artifact() {\n    let request_fingerprint = \"prompt-output\";\n    let _ = request_fingerprint;\n}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "failure",
            "--message",
            "cache_cli::writeback::prompt_output_replay expected hit actual miss request_fingerprint file_hash write_prompt_output_artifact load_prompt_output_artifact",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search failure");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with("[search-failure] kind=test-failure profile=failure-frontier"),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("F=failure:test-failure(cache_cli::writeback::prompt_output_replay)!failure"),
        "{stdout}"
    );
    assert!(
        stdout.contains("O=owner:path(src/cache_cli/writeback.rs)!owner"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "H=hot:fn(write_prompt_output_artifact)@rust://src/cache_cli/writeback.rs#item/fn/write_prompt_output_artifact!code"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "H2=hot:fn(load_prompt_output_artifact)@rust://src/cache_cli/writeback.rs#item/fn/load_prompt_output_artifact!code"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("hot:fn(request_fingerprint)"),
        "evidence variables should not become hot code selectors: {stdout}"
    );
    assert!(
        stdout.contains(
            "frontierActions=C1.query-code(selector=rust://src/cache_cli/writeback.rs#item/fn/write_prompt_output_artifact,owner=src/cache_cli/writeback.rs,symbol=write_prompt_output_artifact,source=H,language=rust)!query-code,C2.query-code(selector=rust://src/cache_cli/writeback.rs#item/fn/load_prompt_output_artifact,owner=src/cache_cli/writeback.rs,symbol=load_prompt_output_artifact,source=H2,language=rust)!query-code"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("H=hot:fn(write_prompt_output_artifact)@src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("frontierActions=C1.query-code(selector=src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=manual-window-scan,duplicate-read,raw-read,broad-lexical"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "search failure fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_failure_from_last_check_reads_cache_artifact() {
    let root = temp_project_root("search-failure-from-last-check-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create source");
    std::fs::create_dir_all(cache_home.join("agent-semantic-protocol")).expect("create cache");
    std::fs::write(
        root.join("src/cache_cli/probe.rs"),
        "fn probe_generation_hit() {\n    let request_fingerprint = \"prompt-output\";\n    let _ = request_fingerprint;\n}\n",
    )
    .expect("write source");
    std::fs::write(
        cache_home
            .join("agent-semantic-protocol")
            .join("last-check-output.txt"),
        "cache_cli::probe::replay expected hit actual miss request_fingerprint probe_generation_hit",
    )
    .expect("write last check");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "search",
            "failure",
            "--from-last-check",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search failure --from-last-check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("F=failure:test-failure(cache_cli::probe::replay)!failure"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "H=hot:fn(probe_generation_hit)@rust://src/cache_cli/probe.rs#item/fn/probe_generation_hit!code"
        ),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "search failure from-last-check should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_failure_can_emit_graph_turbo_request_debug_view() {
    let root = temp_project_root("search-failure-graph-turbo-request-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create source");
    std::fs::write(
        root.join("src/cache_cli/writeback.rs"),
        "fn write_prompt_output_artifact() {\n    let request_fingerprint = \"prompt-output\";\n    let _ = request_fingerprint;\n}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "failure",
            "--message",
            "cache_cli::writeback::prompt_output_replay request_fingerprint write_prompt_output_artifact",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search failure --view graph-turbo-request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(!stdout.starts_with("[search-failure]"), "{stdout}");
    assert!(
        stdout.contains("\"packetKind\": \"graph-turbo-request\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"profile\": \"failure-frontier\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"boundarySource\": \"syntax-header-scan\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("\"matchedTerm\": \"request_fingerprint\"")
            || stdout.contains("\"matchedTerm\": \"write_prompt_output_artifact\""),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "search failure graph-turbo-request fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn check_changed_view_seeds_projects_failure_frontier() {
    let root = temp_project_root("check-changed-failure-frontier-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create source");
    std::fs::write(
        root.join("src/cache_cli/writeback.rs"),
        "fn write_prompt_output_artifact() {\n    let request_fingerprint = \"prompt-output\";\n    let file_hash = request_fingerprint;\n    let _ = file_hash;\n}\n",
    )
    .expect("write source");
    write_check_failure_provider(
        &bin_dir,
        "rs-harness",
        "cache_cli::writeback::prompt_output_replay expected hit actual miss request_fingerprint file_hash write_prompt_output_artifact",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "check", "changed", "--view", "seeds", "."])
        .output()
        .expect("run asp rust check changed --view seeds");

    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stdout.starts_with("[search-failure] kind=test-failure profile=failure-frontier"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "H=hot:fn(write_prompt_output_artifact)@rust://src/cache_cli/writeback.rs#item/fn/write_prompt_output_artifact!code"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "frontierActions=C1.query-code(selector=rust://src/cache_cli/writeback.rs#item/fn/write_prompt_output_artifact,owner=src/cache_cli/writeback.rs,symbol=write_prompt_output_artifact,source=H,language=rust)!query-code"
        ),
        "{stdout}"
    );
    assert!(
        !stdout.contains("H=hot:fn(write_prompt_output_artifact)@src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("frontierActions=C1.query-code(selector=src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(stdout.contains("frontier=A.evidence,H.code"), "{stdout}");
    assert!(
        !stdout.contains("frontier=F.failure"),
        "failure seed should not be the prompt next-action frontier: {stdout}"
    );
    assert!(
        !stderr.contains("unexpected --view") && !stderr.contains("missing --changed"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
