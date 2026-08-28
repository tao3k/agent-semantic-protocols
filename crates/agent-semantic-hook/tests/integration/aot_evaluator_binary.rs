use std::process::Command;

#[test]
fn inherited_no_agent_bypasses_before_generation_or_payload_load() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp-hook-evaluator"))
        .env("ASP_NO_AGENT", "1")
        .arg("--generation")
        .arg("/path/that/must/not/be-read")
        .arg("--host-match")
        .arg("Bash")
        .output()
        .expect("launch AOT Hook evaluator");

    assert!(output.status.success());
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed bypass receipt");
    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(receipt["decision"], "allow");
    assert_eq!(receipt["reasonKind"], "process-no-agent-bypass");
    assert_eq!(receipt["terminal"], "bypassed-before-hook-generation-load");
}
