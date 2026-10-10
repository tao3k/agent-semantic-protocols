// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{live_corpus_receipts, parse_args, run};
use crate::command::live_corpus::qualification::AgentOrgTopologyEvidence;

#[test]
fn missing_agent_response_is_a_typed_semantic_topology_failure() {
    let error = parse_args(&["--resource".to_owned(), "rust.bytes".to_owned()])
        .expect_err("an Agent response is required");
    assert!(error.contains("reasonKind=semantic-topology-not-materialized"));
}

#[test]
fn exact_evidence_and_external_agent_result_publish_an_overlay() {
    let state = tempfile::tempdir().expect("state home");
    let evidence = AgentOrgTopologyEvidence::admit(
        "rust.bytes.core".to_owned(),
        "rust.bytes".to_owned(),
        digest('1'),
        digest('2'),
        "rust://src/lib.rs#item/function/run".to_owned(),
        vec![
            "rust://src/lib.rs#item/function/run".to_owned(),
            "rust://src/runtime.rs#item/function/dispatch".to_owned(),
        ],
        "search-composed-1".to_owned(),
        "(search (producers (language rust)) (intersect (rg \"run\") (tantivy \"run\")))",
        "query-source-1".to_owned(),
        vec![
            (
                "rust://src/lib.rs#item/function/run".to_owned(),
                b"pub fn run() {}\n".to_vec(),
            ),
            (
                "rust://src/runtime.rs#item/function/dispatch".to_owned(),
                b"pub fn dispatch() {}\n".to_vec(),
            ),
        ],
        "query-skeleton-1".to_owned(),
        vec![
            (
                "rust://src/lib.rs#item/function/run".to_owned(),
                br#"{"schemaId":"agent.semantic-protocols.callable-skeleton"}"#.to_vec(),
            ),
            (
                "rust://src/runtime.rs#item/function/dispatch".to_owned(),
                br#"{"schemaId":"agent.semantic-protocols.callable-skeleton"}"#.to_vec(),
            ),
        ],
        "Analyze exact evidence and return an Org topology contribution.".to_owned(),
        vec!["dispatches-to".to_owned()],
    )
    .expect("evidence");
    let evidence_path = live_corpus_receipts(state.path())
        .join("agent-org-topology-evidence/by-resource/rust.bytes.json");
    std::fs::create_dir_all(evidence_path.parent().expect("evidence parent"))
        .expect("evidence directory");
    std::fs::write(
        &evidence_path,
        serde_json::to_vec(&evidence).expect("encode evidence"),
    )
    .expect("write evidence");
    let contribution_path = state.path().join("agent-response.json");
    std::fs::write(
        &contribution_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.agent-org-topology-contribution",
            "schemaVersion": "1",
            "caseId": "rust.bytes.core",
            "resourceId": "rust.bytes",
            "evidenceDigest": evidence.evidence_digest.clone(),
            "agentIdentityDigest": digest('3'),
            "promptDigest": evidence.prompt_digest.clone(),
            "summary": "run dispatches the admitted request",
            "orgSource": "* Agent summary\n** Relationship\nrun dispatches the request.\n",
            "relationships": [{
                "fromSelector": "rust://src/lib.rs#item/function/run",
                "relation": "dispatches-to",
                "toSelector": "rust://src/runtime.rs#item/function/dispatch"
            }]
        }))
        .expect("encode contribution"),
    )
    .expect("write contribution");
    run(
        &[
            "--resource".to_owned(),
            "rust.bytes".to_owned(),
            "--agent-response".to_owned(),
            contribution_path.display().to_string(),
        ],
        state.path(),
    )
    .expect("topology qualification");
    let overlay_path = live_corpus_receipts(state.path())
        .join("agent-org-topology-overlay/by-resource/rust.bytes.json");
    let overlay: agent_semantic_topology::AgentOrgTopologyOverlay =
        serde_json::from_slice(&std::fs::read(overlay_path).expect("published overlay"))
            .expect("decode overlay");
    overlay.validate().expect("self-validating overlay");
    assert_eq!(overlay.evidence_digest, evidence.evidence_digest);
}

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}
