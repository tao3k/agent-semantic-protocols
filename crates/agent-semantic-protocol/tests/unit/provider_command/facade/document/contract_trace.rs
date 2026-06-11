use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn org_facade_exposes_contract_trace_json_without_runtime_verdict() {
    let root = temp_project_root("org-document-contract-trace");
    std::fs::write(root.join("contract.org"), contract_source()).expect("write contract");
    std::fs::write(root.join("notes.org"), target_source()).expect("write notes");

    let output = asp_command(&root)
        .args([
            "org",
            "contract",
            "trace",
            "--org-contract-registry",
            "contract.org",
            "notes.org",
        ])
        .output()
        .expect("run asp org contract trace");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse contract trace");
    let assertion = &packet["files"][0]["evaluations"][0]["assertions"][0];
    assert_eq!(packet["schemaVersion"], 1);
    assert_eq!(
        packet["files"][0]["evaluations"][0]["contractId"],
        "agent.evidence-link-task.v1"
    );
    assert_eq!(assertion["assertionId"], "task.evidence-has-link");
    assert_eq!(assertion["status"], "passed");
    assert_eq!(assertion["actualCount"], 1);
    assert!(
        assertion["matchedIds"]
            .as_array()
            .expect("matched ids")
            .len()
            == 1
    );
    assert!(
        assertion["bindings"]["evidence"]
            .as_array()
            .expect("evidence binding")
            .len()
            == 1
    );
    assert!(packet.get("score").is_none());
    assert!(packet.get("verdict").is_none());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn md_facade_rejects_contract_trace() {
    let root = temp_project_root("md-document-contract-trace");

    let output = asp_command(&root)
        .args(["md", "contract", "trace"])
        .output()
        .expect("run asp md contract trace");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported document command `contract`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("supported commands are guide|search|query|elements-query"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn contract_source() -> &'static str {
    r#"* evidence-link-task-v1
:PROPERTIES:
:CONTRACT_ID: agent.evidence-link-task.v1
:CONTRACT_SCOPE: subtree
:CONTRACT_KIND: org-elements
:END:

** evidence-has-link
:PROPERTIES:
:ASSERT_ID: task.evidence-has-link
:SEVERITY: warning
:END:

#+BEGIN_SRC org-contract
let evidence = headline where child_of($scope) and property(:raw-value) = "Evidence"

assert count link where
  descendant_of(evidence)
>= 1
#+END_SRC
"#
}

fn target_source() -> &'static str {
    r#"* Task A
:PROPERTIES:
:CONTRACT_ORG: ./contract.org#agent.evidence-link-task.v1
:END:
** Evidence
[[https://example.test][inside]]
"#
}
