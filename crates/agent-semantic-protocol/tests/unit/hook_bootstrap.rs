use super::{event_is_observational, hook_event, is_contract_drift, validate_hook_args};
use std::ffi::OsString;
use std::process::{ExitStatus, Output};

#[cfg(unix)]
fn failed_output(stderr: &str) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output {
        status: ExitStatus::from_raw(2 << 8),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[cfg(unix)]
#[test]
fn bootstrap_recognizes_only_typed_freshness_failures() {
    assert!(is_contract_drift(&failed_output(
        "hook matcher config freshness gate failed: unknown variant"
    )));
    assert!(is_contract_drift(&failed_output(
        "hook resident config freshness gate failed"
    )));
    assert!(!is_contract_drift(&failed_output(
        "provider execution failed: unknown variant"
    )));
}

#[test]
fn lifecycle_failures_degrade_open_but_enforcement_failures_do_not() {
    for event in [
        "session-start",
        "user-prompt",
        "post-tool",
        "subagent-start",
        "subagent-stop",
        "stop",
    ] {
        let args = vec![OsString::from("hook"), OsString::from(event)];
        assert!(event_is_observational(&args), "{event}");
    }
    for event in ["pre-tool", "permission-request"] {
        let args = vec![OsString::from("hook"), OsString::from(event)];
        assert!(!event_is_observational(&args), "{event}");
    }
}

#[test]
fn bootstrap_rejects_non_hook_dispatch() {
    let valid = vec![OsString::from("hook"), OsString::from("stop")];
    assert_eq!(hook_event(&valid), Some("stop"));
    assert!(validate_hook_args(&valid).is_ok());
    assert!(validate_hook_args(&[OsString::from("server"), OsString::from("stop")]).is_err());
}

#[test]
fn unavailable_server_activation_falls_back_to_the_local_workspace() {
    for output in [
        r#"{"reasonKind":"activation-unavailable"}"#,
        r#"{"additionalContext":"{\"reasonKind\":\"activation-unavailable\"}"}"#,
    ] {
        assert!(
            server_hook_output_requires_local_fallback(output),
            "{output}"
        );
    }
    assert!(!server_hook_output_requires_local_fallback(
        r#"{"reasonKind":"structured-source-read"}"#
    ));
}
use super::server_hook_output_requires_local_fallback;
