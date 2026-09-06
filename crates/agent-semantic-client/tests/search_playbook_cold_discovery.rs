use std::process::Command;

#[test]
fn layer_one_discovery_does_not_require_a_runtime_endpoint_or_activation() {
    let state_home = tempfile::tempdir().expect("create isolated ASP State Home");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "search",
            "playbook",
            "--languages",
            "rust",
            "--fd",
            "runtime_server",
            "--rg",
            "RuntimeClientHandoff|runtime-client-handoff-unavailable",
            "--tantivy",
            "runtime status handoff endpoint observation",
            "--workspace",
        ])
        .arg(workspace)
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_RUNTIME_CLIENT_FD")
        .output()
        .expect("launch current-tree Search Playbook");

    let terminal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "Layer One discovery must remain available without Runtime activation: {terminal}"
    );
    assert!(
        !terminal.contains("endpoint.v1.json") && !terminal.contains("applied-activation-required"),
        "Layer One discovery must not consult a retired serving authority: {terminal}"
    );
    assert!(
        terminal.contains("#+title: ASP Search Playbook Layer One Evidence")
            && terminal.contains("#+property: SELECTOR_AUTHORITY none")
            && terminal.contains("#+property: CONTENT_GENERATION blake3-256:"),
        "Layer One must return a typed Org evidence projection: {terminal}"
    );
    assert!(
        terminal.contains("runtime_server")
            && terminal.contains("Use parser-owned syntax evidence to obtain canonical selectors"),
        "Layer One must return actionable owner evidence without inventing selectors: {terminal}"
    );
}

#[test]
fn syntax_enrichment_still_requires_runtime_handoff_without_endpoint_fallback() {
    let state_home = tempfile::tempdir().expect("create isolated ASP State Home");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "search",
            "playbook",
            "--languages",
            "rust",
            "--rg",
            "RuntimeClientHandoff",
            "--syntax",
            "rust",
            "items",
            "--workspace",
        ])
        .arg(workspace)
        .env("ASP_STATE_HOME", state_home.path())
        .env_remove("ASP_RUNTIME_CLIENT_FD")
        .output()
        .expect("launch Search Playbook syntax enrichment");

    let terminal = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "syntax projection requires Runtime"
    );
    assert!(
        terminal.contains("reasonKind=runtime-client-handoff-unavailable"),
        "syntax projection must fail at the Host handoff boundary: {terminal}"
    );
    assert!(
        !terminal.contains("endpoint.v1.json"),
        "syntax projection must not rederive a legacy endpoint: {terminal}"
    );
}
