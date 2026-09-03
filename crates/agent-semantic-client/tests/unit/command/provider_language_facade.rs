use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn structural_item_source_query_does_not_use_cli_breaker() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "rust",
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
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("run public exact-query facade");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "public exact query failed: stdout={stdout} stderr={stderr}"
    );
    assert!(
        !stderr.contains("provider command must be admitted as a Runtime Server route"),
        "public query must cross Runtime admission instead of the removed CLI breaker: {stderr}"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "public query must stop at the exact isolated Runtime authority terminal: {stdout}"
    );
}

#[test]
fn gerbil_owner_source_query_enters_runtime_instead_of_the_cli_breaker() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "gerbil-scheme",
            "query",
            "--selector",
            "scheme/reasoning/core.ss",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_NO_AGENT")
        .output()
        .expect("run public Gerbil owner-source query");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Gerbil owner-source query failed: stdout={stdout} stderr={stderr}"
    );
    assert!(
        !stderr.contains("provider command must be admitted as a Runtime Server route"),
        "Gerbil owner query was rejected before Runtime admission: {stderr}"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "owner query must stop at the isolated Runtime authority: {stdout}"
    );
}

#[test]
fn public_search_facades_use_runtime_admission_for_registered_languages() {
    for language_id in ["rust", "python", "gerbil-scheme"] {
        let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                language_id,
                "search",
                "pipe",
                "RuntimeAspClient",
                "--workspace",
                ".",
            ])
            .current_dir(workspace_root())
            .env("ASP_STATE_HOME", state_home.path())
            .output()
            .unwrap_or_else(|error| panic!("run public {language_id} search facade: {error}"));
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("provider command must be admitted as a Runtime Server route"),
            "public {language_id} search must cross Runtime admission: {stderr}"
        );
        assert!(
            stderr.is_empty(),
            "unexpected {language_id} stderr: {stderr}"
        );
        assert!(
            stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
            "public {language_id} search must stop at the exact isolated Runtime authority terminal: {stdout}"
        );
    }
}

#[test]
fn structural_item_source_query_requires_current_runtime_authority_before_provider_resolution() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr={stderr}");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "stdout={stdout}"
    );
}

#[test]
fn exact_structural_selector_does_not_require_a_term() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let selector = "typescript://languages/asp-typescript/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery";
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr={stderr}");
    assert!(
        !stderr.contains("query requires at least one --term"),
        "stderr={stderr}"
    );
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "stdout={stdout}"
    );
}

#[test]
fn removed_exact_query_flags_are_rejected_by_cli_admission() {
    for removed_flag in ["--code", "--names-only"] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "typescript",
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
            stderr.contains("unexpected argument"),
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
            "org",
            "query",
            "--selector",
            "org://docs/missing.org#item/heading/missing",
            "--workspace",
            ".",
            "--projection",
            "content",
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
fn language_query_never_claims_runtime_lifecycle_authority() {
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
            "rust",
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
        output_text.contains("ASP Server endpoint is unavailable"),
        "unexpected transport terminal: {output_text}"
    );
    assert!(output_text.contains("\"reasonKind\":\"transport-unavailable\""));
    assert!(!output_text.contains("runtime-server-activation"));
    assert!(
        !temporary
            .path()
            .join(".agent-semantic-protocols/runtime/server/owner-spawn.v1.json")
            .exists(),
        "a language client must not claim Runtime lifecycle ownership"
    );
}
