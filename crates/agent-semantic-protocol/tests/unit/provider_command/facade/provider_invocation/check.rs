use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_check_failure_provider,
};

#[test]
fn check_changed_view_seeds_renders_failure_frontier_after_provider_failure() {
    let root = temp_project_root("provider-check-failure-frontier-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/cache_cli")).expect("create src dir");
    std::fs::write(
        root.join("src/cache_cli/writeback.rs"),
        "pub fn write_prompt_output_artifact() {\n    let request_fingerprint = \"miss\";\n    let file_hash = request_fingerprint;\n    assert!(!file_hash.is_empty());\n}\n",
    )
    .expect("write source");
    write_check_failure_provider(
        &bin_dir,
        "rs-harness",
        "cache_cli::write_prompt_output_artifact expected hit actual miss\nrequest_fingerprint file_hash\n",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "check", "changed", "--view", "seeds", "."])
        .output()
        .expect("run asp rust check changed seeds");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with(
            "[search-failure] kind=test-failure profile=failure-frontier alg=typed-ppr-diverse"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("F=failure:test-failure(")
            && stdout.contains("write_prompt_output_artifact"),
        "{stdout}"
    );
    assert!(
        stdout.contains("frontierActions=")
            && stdout.contains("C1.query-code(selector=src/cache_cli/writeback.rs:1:5"),
        "{stdout}"
    );
    assert!(
        stdout.contains("queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "entries=failure-frontier(F=>failure-facts+candidate-owners+hot-blocks+query-profiles)"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("omit=full-source,unrelated-functions,wide-windows"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=manual-window-scan,duplicate-read,raw-read,broad-fzf"),
        "{stdout}"
    );
    for debug_prefix in [
        "scores=", "paths=", "cache=", "trace=", "explain=", "metrics=",
    ] {
        assert!(
            !stdout.lines().any(|line| line.starts_with(debug_prefix)),
            "{stdout}"
        );
    }
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(!stderr.contains("unexpected --view"), "{stderr}");
    let last_check =
        std::fs::read_to_string(root.join(".cache/agent-semantic-protocol/last-check-output.txt"))
            .expect("last check output");
    assert!(
        last_check.contains("cache_cli::write_prompt_output_artifact"),
        "{last_check}"
    );
    let _ = std::fs::remove_dir_all(root);
}
