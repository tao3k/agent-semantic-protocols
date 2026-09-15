// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Test-owned admission of an external Agent result into an Org topology overlay.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::qualification::AgentOrgTopologyEvidence;

#[derive(Debug)]
struct Args {
    resource_id: String,
    agent_response: PathBuf,
    json: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentTopologyContribution {
    schema_id: String,
    schema_version: String,
    case_id: String,
    resource_id: String,
    evidence_digest: String,
    agent_identity_digest: String,
    prompt_digest: String,
    summary: String,
    org_source: String,
    relationships: Vec<agent_semantic_topology::AgentTopologyRelationship>,
}

pub(super) fn run(args: &[String], resource_state_home: &Path) -> Result<(), String> {
    let args = parse_args(args)?;
    let evidence_path = live_corpus_receipts(resource_state_home)
        .join("agent-org-topology-evidence/by-resource")
        .join(format!("{}.json", args.resource_id));
    let evidence = read_json::<AgentOrgTopologyEvidence>(&evidence_path, "topology evidence")?;
    evidence.validate()?;
    let contribution = read_json::<AgentTopologyContribution>(
        &args.agent_response,
        "external Agent topology contribution",
    )?;
    if contribution.schema_id != "agent.semantic-protocols.agent-org-topology-contribution"
        || contribution.schema_version != "1"
        || contribution.case_id != evidence.case_id
        || contribution.resource_id != evidence.resource_id
        || contribution.resource_id != args.resource_id
        || contribution.evidence_digest != evidence.evidence_digest
        || contribution.prompt_digest != evidence.prompt_digest
    {
        return Err("reasonKind=semantic-topology-agent-contribution-binding-mismatch".to_owned());
    }
    let overlay = agent_semantic_topology::AgentOrgTopologyOverlay::admit(
        evidence.source_generation_digest.clone(),
        evidence.base_topology_generation_digest.clone(),
        evidence.selector.clone(),
        evidence.evidence_digest.clone(),
        contribution.agent_identity_digest,
        evidence.prompt_digest.clone(),
        contribution.summary,
        contribution.org_source,
        contribution.relationships,
    )
    .map_err(|error| format!("admit external Agent Org topology overlay: {error}"))?;
    overlay
        .validate_against_base(
            &evidence.source_generation_digest,
            &evidence.base_topology_generation_digest,
        )
        .map_err(|error| format!("join external Agent Org topology overlay: {error}"))?;
    let encoded = serde_json::to_vec(&overlay)
        .map_err(|error| format!("encode Agent Org topology overlay: {error}"))?;
    let output = live_corpus_receipts(resource_state_home)
        .join("agent-org-topology-overlay/by-resource")
        .join(format!("{}.json", args.resource_id));
    publish_atomically(&output, &encoded)?;
    if args.json {
        println!(
            "{}",
            std::str::from_utf8(&encoded)
                .map_err(|error| format!("render Agent Org topology overlay: {error}"))?
        );
    } else {
        println!(
            "[live-corpus-topology] resource={} evidenceDigest={} overlayDigest={} receipt={} status=admitted",
            args.resource_id,
            evidence.evidence_digest,
            overlay.overlay_digest,
            output.display()
        );
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut resource_id = None;
    let mut agent_response = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--resource" => {
                index += 1;
                resource_id = args.get(index).cloned();
            }
            "--agent-response" => {
                index += 1;
                agent_response = args.get(index).map(PathBuf::from);
            }
            "--json" => json = true,
            option => return Err(format!("unknown Live Corpus topology option: {option}")),
        }
        index += 1;
    }
    Ok(Args {
        resource_id: resource_id.filter(|value| !value.is_empty()).ok_or_else(|| {
            "reasonKind=semantic-topology-not-materialized missing Live Corpus resource identity"
                .to_owned()
        })?,
        agent_response: agent_response.ok_or_else(|| {
            "reasonKind=semantic-topology-not-materialized missing external Agent response"
                .to_owned()
        })?,
        json,
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!("reasonKind=semantic-topology-not-materialized read {label}: {error}")
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!("reasonKind=semantic-topology-not-materialized decode {label}: {error}")
    })
}

fn live_corpus_receipts(state_home: &Path) -> PathBuf {
    agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .resources()
        .live_corpus()
        .join("receipts")
}

fn publish_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Agent Org topology overlay has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Agent Org topology receipt directory: {error}"))?;
    let temporary = parent.join(format!(
        ".agent-org-topology-overlay.{}.tmp",
        std::process::id()
    ));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write Agent Org topology overlay: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish Agent Org topology overlay: {error}"))
}

#[cfg(test)]
mod tests {
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
            "query-source-1".to_owned(),
            b"pub fn run() {}\n",
            "query-skeleton-1".to_owned(),
            br#"{"schemaId":"agent.semantic-protocols.callable-skeleton"}"#,
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
}
