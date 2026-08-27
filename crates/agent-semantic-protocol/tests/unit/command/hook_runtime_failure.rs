use super::observe_hook_execution;

#[tokio::test]
async fn panicking_hook_future_becomes_a_typed_observable_terminal() {
    let error = observe_hook_execution(
        Some("pre-tool".to_owned()),
        Some("codex".to_owned()),
        async {
            panic!("reactor authority regression");
            #[allow(unreachable_code)]
            Ok(())
        },
    )
    .await
    .expect_err("panic must not escape the Hook execution boundary");
    let receipt: serde_json::Value = serde_json::from_str(&error).expect("typed Hook failure JSON");

    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.hook.execution-failure"
    );
    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(receipt["state"], "failed");
    assert_eq!(receipt["phase"], "bootstrap");
    assert_eq!(receipt["failureKind"], "runtime-panic");
    assert_eq!(receipt["event"], "pre-tool");
    assert_eq!(receipt["client"], "codex");
    assert!(
        receipt["runtimeArtifactFingerprint"]
            .as_str()
            .is_some_and(|fingerprint| !fingerprint.is_empty())
    );
    assert!(
        receipt["message"]
            .as_str()
            .is_some_and(|message| message.contains("reactor authority regression"))
    );
}

#[tokio::test]
async fn hook_runtime_error_uses_the_same_typed_terminal_channel() {
    let error = observe_hook_execution(
        Some("pre-tool".to_owned()),
        Some("codex".to_owned()),
        async { Err("matcher snapshot unavailable".to_owned()) },
    )
    .await
    .expect_err("runtime error must become typed Hook failure");
    let receipt: serde_json::Value = serde_json::from_str(&error).expect("typed Hook failure JSON");
    assert_eq!(receipt["failureKind"], "runtime-error");
    assert_eq!(receipt["message"], "matcher snapshot unavailable");
}
