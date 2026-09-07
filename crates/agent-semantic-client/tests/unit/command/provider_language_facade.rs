// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

fn assert_runtime_activation_missing(output: &std::process::Output, operation: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded: {text}"
    );
    assert!(
        text.contains("state=runtime-server-client-bootstrap-failed")
            && text.contains("reasonKind=activation-event-missing"),
        "{operation} did not reach the isolated Runtime authority: {text}"
    );
    assert!(
        !text.contains("language-first")
            && !text.contains("provider command must be admitted as a Runtime Server route"),
        "{operation} was rejected before Runtime admission: {text}"
    );
}

fn assert_runtime_transport_missing(output: &std::process::Output, operation: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded: {text}"
    );
    assert!(
        text.contains("\"failureLayer\":\"runtime-transport-capability\"")
            && text.contains("\"reasonKind\":\"transport-unavailable\""),
        "{operation} did not fail at the Host transport boundary: {text}"
    );
    assert!(
        !text.contains("language-first")
            && !text.contains("provider command must be admitted as a Runtime Server route"),
        "{operation} was rejected before Runtime transport admission: {text}"
    );
}

#[test]
fn structural_item_source_query_does_not_use_cli_breaker() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "--selector",
            "rust://crates/agent-semantic-client/src/command/provider_dispatch.rs#item/function/run_language_command",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run public exact-query facade");

    assert_runtime_activation_missing(&output, "public exact query");
}

#[test]
fn gerbil_owner_source_query_enters_runtime_instead_of_the_cli_breaker() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "--selector",
            "gerbil-scheme://scheme/reasoning/core.ss#item/function/missing",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run public Gerbil owner-source query");

    assert_runtime_activation_missing(&output, "Gerbil exact query");
}

#[test]
fn search_playbook_uses_runtime_admission_for_registered_languages() {
    for language_id in ["rust", "python", "gerbil-scheme"] {
        let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "search",
                "playbook",
                "--languages",
                language_id,
                "--workspace",
                ".",
            ])
            .current_dir(workspace_root())
            .env("ASP_STATE_HOME", state_home.path())
            .output()
            .unwrap_or_else(|error| panic!("run public {language_id} search facade: {error}"));
        assert_runtime_transport_missing(&output, &format!("{language_id} Search Playbook"));
    }
}

#[test]
fn structural_item_source_query_requires_current_runtime_authority_before_provider_resolution() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "--selector",
            "typescript://languages/asp-typescript/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run asp structural item query");

    assert_runtime_activation_missing(&output, "TypeScript exact query");
}

#[test]
fn exact_structural_selector_does_not_require_a_term() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let selector = "typescript://languages/asp-typescript/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery";
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "--selector",
            selector,
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run exact structural selector query");

    assert_runtime_activation_missing(&output, "term-free exact query");
}

#[test]
fn removed_exact_query_flags_are_rejected_by_cli_admission() {
    for removed_flag in ["--code", "--names-only"] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "query",
                "--selector",
                "typescript://languages/asp-typescript/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
                "--workspace",
                ".",
                removed_flag,
            ])
            .current_dir(workspace_root())
            .output()
            .expect("run removed exact-query flag");

        assert!(!output.status.success(), "{removed_flag} was admitted");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("query does not support option"),
            "removed flag did not use ordinary CLI rejection: flag={removed_flag} stderr={stderr}"
        );
        assert!(
            !stderr.contains("legacy") && !stderr.contains("unsupported"),
            "removed flag leaked compatibility guidance: flag={removed_flag} stderr={stderr}"
        );
    }
}

#[test]
fn document_exact_selector_crosses_the_language_neutral_owner_boundary() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "query",
            "--selector",
            "org://docs/missing.org#item/heading/missing",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run org exact structural selector query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("provider-owned structural query is missing an exact owner path"),
        "org selector was rejected by a language-hardcoded owner boundary: {stderr}"
    );
    assert!(
        !stderr.contains("query requires at least one --term"),
        "org exact selector was misrouted into lexical query: {stderr}"
    );
}

#[test]
fn language_query_reports_missing_activation_through_client_bootstrap() {
    let temporary = tempfile::tempdir().expect("isolated client State Home");
    let project_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&project_root).expect("create fixture workspace");
    std::fs::create_dir_all(temporary.path().join(".agent-semantic-protocols"))
        .expect("create empty State Home");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .env_remove("ASP_STATE_HOME")
        .env_remove("ASP_RUNTIME_CLIENT_FD")
        .env("HOME", temporary.path())
        .current_dir(&project_root)
        .args([
            "query",
            "--selector",
            "rust://src/lib.rs#item/function/missing",
            "--projection",
            "source",
            "--workspace",
        ])
        .arg(&project_root)
        .output()
        .expect("run language query without a published transport");

    let output_text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success());
    assert!(
        output_text.contains("state=runtime-server-client-bootstrap-failed"),
        "language query must report the lifecycle acquisition terminal: {output_text}"
    );
    assert!(output_text.contains("reasonKind=activation-event-missing"));
    assert!(
        !temporary
            .path()
            .join(".agent-semantic-protocols/runtime/server/owner-spawn.v1.json")
            .exists(),
        "client bootstrap must not fabricate an owner without an activation"
    );
}
