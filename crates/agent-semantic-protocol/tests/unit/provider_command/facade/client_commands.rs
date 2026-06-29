use serde_json::json;

use crate::provider_command::support::{
    artifacts_root, asp_command, make_executable, prepend_path, temp_project_root,
};

#[test]
fn top_level_guide_lists_active_language_contract_and_provider_axes() {
    let root = temp_project_root("top-level-guide-language-contract");

    let output = asp_command(&root)
        .args(["guide"])
        .output()
        .expect("run asp guide");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|cmd guide=asp <language> guide --workspace ."),
        "{stdout}"
    );
    assert!(
        stdout.contains("known=rust|typescript|python|julia|gerbil-scheme|org|md"),
        "{stdout}"
    );
    assert!(
        stdout.contains("provider-guide-contract=run asp <language> guide --workspace ."),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "provider-knowledge-axes=asp <language> search env|runtime-source|lang|std|capability|extension|pattern|compare"
        ),
        "{stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tools_doctor_reports_required_external_tools() {
    let root = temp_project_root("tools-doctor");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    for tool in ["fd", "rg", "fzf", "eza", "asp-graph-turbo"] {
        let path = bin_dir.join(tool);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write fake tool");
        make_executable(&path);
    }

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["tools", "doctor", "."])
        .output()
        .expect("run asp tools doctor");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[asp-tools] status=ok"));
    assert!(stdout.contains("required=fd,rg,fzf,eza,asp-graph-turbo"));
    assert!(stdout.contains("|tool name=fd status=ok"));
    assert!(stdout.contains("|tool name=rg status=ok"));
    assert!(stdout.contains("|tool name=fzf status=ok"));
    assert!(stdout.contains("|tool name=eza status=ok"));
    assert!(stdout.contains("|tool name=asp-graph-turbo status=ok path="));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn wrap_graph_turbo_is_native_top_level_command() {
    let root = temp_project_root("wrap-graph-turbo");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let marker = root.join("wrapped-args.txt");
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    std::fs::write(
        &graph_turbo,
        "#!/bin/sh\n\
         printf '%s\n' \"$@\" > \"$1\"\n",
    )
    .expect("write fake asp-graph-turbo");
    make_executable(&graph_turbo);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "wrap",
            "asp-graph-turbo",
            "--",
            &marker.display().to_string(),
            "rank",
        ])
        .output()
        .expect("run asp wrap graph turbo");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let observed = std::fs::read_to_string(&marker).expect("read wrapped args");
    assert_eq!(observed, format!("{}\nrank\n", marker.display()));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_history_audit_reads_prompt_output_command_artifacts() {
    let root = temp_project_root("search-history-audit");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let expected_artifacts_root = artifacts_root(&root);
    std::fs::create_dir_all(&expected_artifacts_root).expect("create expected artifacts root");
    let expected_artifacts_root_canonical = expected_artifacts_root
        .canonicalize()
        .unwrap_or_else(|_| expected_artifacts_root.clone());
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    std::fs::write(
        &graph_turbo,
        format!(
            "#!/bin/sh\n\
         test \"$1\" = timeline || {{ echo expected timeline >&2; exit 2; }}\n\
         test \"$2\" = '{}' || test \"$2\" = '{}' || {{ echo bad artifact dir >&2; exit 2; }}\n\
         echo '[graph-turbo-timeline] fake=true events=3 actions=3 sessions=1 rounds=1'\n\
         echo '[graph-turbo-owner-collapse] collapsible=1 actions=1'\n\
         echo '[graph-turbo-owner-action] decision=collapse replacement=promote-to-owner-query-item-test-frontier'\n",
            expected_artifacts_root.display(),
            expected_artifacts_root_canonical.display()
        ),
    )
    .expect("write fake asp-graph-turbo");
    make_executable(&graph_turbo);
    let artifact_dir = artifacts_root(&root).join("prompt-output");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    write_command_artifact(
        &artifact_dir.join("rust-search-owner-a.command.json"),
        json!({
            "providerCommands": [{
                "argv": ["/tmp/rs-harness", "search", "owner", "src/lib.rs", "items", "--query", "cacheRoot|ClientReceipt", "--view", "seeds"],
                "elapsedMs": 40,
                "exitCode": 0,
                "languageId": "rust",
                "providerId": "rs-harness",
                "stdoutBytes": 1,
                "stderrBytes": 0
            }]
        }),
    );
    write_command_artifact(
        &artifact_dir.join("rust-search-owner-b.command.json"),
        json!({
            "providerCommands": [{
                "argv": ["/tmp/rs-harness", "search", "owner", "src/lib.rs", "items", "--query", "cacheRoot|ClientReceipt", "--view", "seeds"],
                "elapsedMs": 80,
                "exitCode": 0,
                "languageId": "rust",
                "providerId": "rs-harness",
                "stdoutBytes": 1,
                "stderrBytes": 0
            }]
        }),
    );
    write_command_artifact(
        &artifact_dir.join("rust-search-fzf.command.json"),
        json!({
            "providerCommands": [{
                "argv": ["/tmp/rs-harness", "search", "fzf", "cacheRoot", "owner", "tests", "--view", "seeds"],
                "elapsedMs": 10,
                "exitCode": 0,
                "languageId": "rust",
                "providerId": "rs-harness",
                "stdoutBytes": 1,
                "stderrBytes": 0
            }]
        }),
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["search", "history", "audit", ".", "--examples", "2"])
        .output()
        .expect("run asp search history audit");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-turbo-timeline] "));
    assert!(stdout.contains("events=3 actions=3"));
    assert!(stdout.contains("[graph-turbo-owner-collapse]"));
    assert!(stdout.contains("collapsible=1"));
    assert!(stdout.contains("[graph-turbo-owner-action]"));

    let _ = std::fs::remove_dir_all(root);
}

fn write_command_artifact(path: &std::path::Path, value: serde_json::Value) {
    std::fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize command artifact"),
    )
    .expect("write command artifact");
}
