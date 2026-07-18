use serde_json::json;
use std::path::Path;

use super::{CandidateAcquisition, admit_search_pipe_candidates};

fn candidate(path: &str) -> super::super::search_pipe_model::Candidate {
    super::super::search_pipe_model::Candidate {
        path: path.to_owned(),
        line: 1,
        end_line: 1,
        symbol: "candidate".to_owned(),
        selector: None,
        text: String::new(),
        source: "source-index".to_owned(),
        confidence: "exact".to_owned(),
    }
}

#[test]
fn rejects_repository_candidates_before_provider_facts_and_rank() {
    let scope = agent_semantic_search::SemanticWorkspaceScope::from_packet(&json!({
        "schemaId": "agent.semantic-protocols.semantic-workspace-scope",
        "schemaVersion": "1",
        "workspaceId": "python:harness",
        "languageId": "python",
        "providerId": "py-harness",
        "packageManager": "uv",
        "sourceExtensions": [".py", ".pyi"],
        "discoveryRoot": "/repo/languages/python-harness",
        "anchors": [{
            "kind": "pyproject",
            "path": "/repo/languages/python-harness/pyproject.toml",
            "sha256": format!("sha256:{}", "a".repeat(64))
        }],
        "packages": [{
            "packageId": "python:harness",
            "name": "harness",
            "root": "/repo/languages/python-harness",
            "manifestPath": "/repo/languages/python-harness/pyproject.toml",
            "languageId": "python"
        }],
        "admittedRoots": ["/repo/languages/python-harness"],
        "fingerprint": format!("sha256:{}", "b".repeat(64))
    }))
    .expect("scope");
    let mut acquisition = CandidateAcquisition {
        candidates: vec![
            candidate("languages/python-harness/src/pkg.py"),
            candidate("crates/agent-semantic-protocol/src/lib.rs"),
        ],
        candidate_sources: vec!["source-index".to_owned()],
        source_trace: Vec::new(),
    };

    admit_search_pipe_candidates(&mut acquisition, &scope, "python", Path::new("/repo"));

    assert_eq!(acquisition.candidates.len(), 1);
    assert_eq!(
        acquisition.candidates[0].path,
        "languages/python-harness/src/pkg.py"
    );
    let trace = acquisition.source_trace.last().expect("scope trace");
    assert_eq!(trace.status, "filtered");
    assert_eq!(trace.matched, 1);
    assert_eq!(trace.missing, 1);
    assert_eq!(
        trace.fields["rejectionKinds"],
        json!(["candidate-out-of-scope"])
    );
}
