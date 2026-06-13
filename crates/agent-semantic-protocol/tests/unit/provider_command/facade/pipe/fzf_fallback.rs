use serde_json::Value;

use super::assert_graph_turbo_request_contract;
use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation,
    write_marker_provider,
};

#[test]
fn fzf_fallback_collector_matches_multiple_terms_without_native_finder() {
    let root = temp_project_root("search-fzf-fallback-multi-term");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let args_path = root.join("graph-turbo-args");
    let stdin_path = root.join("graph-turbo-stdin.json");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/cache_root.rs"),
        "pub fn providerneedle() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("src/providerneedle.txt"),
        "cache_root providerneedle\n",
    )
    .expect("write ignored text source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_graph_turbo_ranker(&bin_dir);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("ASP_GRAPH_TURBO_ARGS_OUT", &args_path)
        .env("ASP_GRAPH_TURBO_STDIN_OUT", &stdin_path)
        .args([
            "rust",
            "search",
            "fzf",
            "cache_root|providerneedle",
            "owner",
            "items",
            "tests",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search fzf without native finder");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "[graph-frontier] external=fzf\n"
    );
    let payload: Value = serde_json::from_slice(&std::fs::read(&stdin_path).expect("read stdin"))
        .expect("graph turbo stdin JSON");
    assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes
            .iter()
            .any(|node| node["kind"] == "item" && node["value"] == "cache_root"),
        "{payload}"
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["kind"] == "item" && node["value"] == "providerneedle"),
        "{payload}"
    );
    assert!(
        nodes.iter().all(|node| {
            !node
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| path.ends_with(".txt"))
        }),
        "{payload}"
    );
    assert!(
        !marker.exists(),
        "fallback fzf seeds should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn write_graph_turbo_ranker(bin_dir: &std::path::Path) {
    std::fs::create_dir_all(bin_dir).expect("create fake graph turbo bin dir");
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    std::fs::write(
        &graph_turbo,
        "#!/bin/sh\n\
         printf '%s\n' \"$@\" > \"$ASP_GRAPH_TURBO_ARGS_OUT\"\n\
         /bin/cat > \"$ASP_GRAPH_TURBO_STDIN_OUT\"\n\
         printf '[graph-frontier] external=fzf\\n'\n",
    )
    .expect("write fake asp-graph-turbo");
    make_executable(&graph_turbo);
}
