use std::io::Write;
use std::process::Stdio;

use serde_json::Value;

use super::assert_graph_turbo_request_contract;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn empty_search_ingest_seeds_is_facade_diagnostic_for_all_languages() {
    let root = temp_project_root("empty-ingest-facade-diagnostic");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let providers = [
        ("rust", "rs-harness"),
        ("typescript", "ts-harness"),
        ("python", "py-harness"),
        ("julia", "asp-julia-harness"),
    ];
    for (_, binary) in providers {
        write_marker_provider(&bin_dir, binary, &marker);
    }
    write_activation(
        &root,
        &[
            provider(
                "rust",
                vec![bin_dir.join("rs-harness").display().to_string()],
            ),
            provider(
                "typescript",
                vec![bin_dir.join("ts-harness").display().to_string()],
            ),
            provider(
                "python",
                vec![bin_dir.join("py-harness").display().to_string()],
            ),
            provider(
                "julia",
                vec![bin_dir.join("asp-julia-harness").display().to_string()],
            ),
        ],
    );

    for (language, _) in providers {
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language, "search", "ingest", "items", "tests", "--view", "seeds", ".",
            ])
            .output()
            .expect("run empty ingest facade");

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.starts_with("[search-ingest]"));
        assert!(stdout.contains("|note kind=stdin-required"));
        assert!(stdout.contains("search prime --workspace . --view seeds"));
        assert!(stdout.contains("|next prime:"));
        assert!(!stdout.contains("test:path(.)"));
        assert!(!stdout.contains("owner:path(search prime"));
        assert!(
            !marker.exists(),
            "empty ingest should not spawn provider for {language}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_ingest_stdin_is_asp_owned_and_does_not_spawn_provider() {
    let root = temp_project_root("provider-stdin-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let mut child = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "search", "ingest", "--view", "seeds", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp rust search ingest");
    child
        .stdin
        .as_mut()
        .expect("facade stdin")
        .write_all(b"src/lib.rs:10:HookDecision\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for facade");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.starts_with("[graph-frontier] profile=owner-query"),
        "{stdout}"
    );
    assert!(stdout.contains("O=owner:path(src/lib.rs)"), "{stdout}");
    assert!(
        stdout.contains("I=item:symbol(hookdecision)@rust://src/lib.rs#item/symbol/hookdecision"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(hookdecision)@src/lib.rs:10:10"),
        "{stdout}"
    );
    assert!(!marker.exists(), "search ingest should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_ingest_can_emit_graph_turbo_request_without_spawning_provider() {
    let root = temp_project_root("provider-stdin-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let mut child = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "ingest",
            "items",
            "tests",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run asp rust search ingest graph turbo request");
    child
        .stdin
        .as_mut()
        .expect("facade stdin")
        .write_all(b"src/lib.rs:10:HookDecision\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for facade");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph turbo request JSON");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["profile"], "owner-query");
    assert_eq!(payload["algorithm"], "typed-ppr-diverse");
    assert_eq!(payload["graph"]["nodes"][0]["kind"], "owner");
    assert!(
        payload["graph"]["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .any(|node| node["kind"] == "item"
                && node["action"] == "syntax"
                && node["syntaxQuery"]
                    .as_str()
                    .is_some_and(|query| !query.is_empty()))
    );
    assert!(
        payload["graph"]["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .any(|edge| edge["relation"] == "covers")
    );
    assert!(!marker.exists(), "search ingest should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
