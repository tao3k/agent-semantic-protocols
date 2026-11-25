// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Deterministic stateful reference-model property tests (200 seeds x 60 steps =
//! 12,000 state transitions) verifying `MarlinConvergedSearchEngine` invariants
//! modeled in `dev/quint/search-engine.qnt`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use agent_semantic_search::{
    InMemorySearchStoragePort, MarlinConvergedSearchEngine, SearchArtifactHash,
    SearchGenerationDigest, SearchProjectId, SearchStoragePortError, SearchTopologyEdgeId,
    SearchTopologyEdgeRecord, SearchWorkspaceId, SourceIndexRankCandidate, SourceIndexRankRequest,
};
use agent_semantic_search_playbook::{
    SearchPlaybookComposition, SearchPlaybookLeaf, SearchPlaybookPlanInput,
};
use agent_semantic_topology::{
    OrgDocumentProjection, OrgDocumentProjectionPort, OrgOverlayDocumentProjection,
    OrgProjectionPortError, OrgPropertyEntry, OrgSectionProjection,
};

struct DeterministicOrgPort;

impl OrgDocumentProjectionPort for DeterministicOrgPort {
    fn project_document(
        &self,
        source: &str,
    ) -> Result<OrgDocumentProjection, OrgProjectionPortError> {
        Ok(OrgDocumentProjection::new(
            OrgDocumentProjection::digest_for_source(source),
            vec![OrgPropertyEntry::new(
                "CONTRACT_ORG",
                "[[../contracts/project.workspace-manifest.v1.org][project.workspace-manifest.v1]]",
            )],
            vec![OrgSectionProjection::new(
                1,
                None,
                vec![
                    OrgPropertyEntry::new("PROJECT_WORKSPACE_ID", "asp-pbt"),
                    OrgPropertyEntry::new(
                        "PROJECT_WORKSPACE_IDENTITY",
                        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
                    ),
                    OrgPropertyEntry::new("WORKSPACE_ROOT_PATH", "."),
                    OrgPropertyEntry::new("PORTABILITY", "cross-machine"),
                    OrgPropertyEntry::new("REPOSITORY_ALIASES", "[]"),
                ],
            )],
        ))
    }

    fn inspect_overlay(
        &self,
        source: &str,
    ) -> Result<OrgOverlayDocumentProjection, OrgProjectionPortError> {
        let digest = format!(
            "blake3-256:{}",
            blake3::hash(format!("asp-pbt:{source}").as_bytes()).to_hex()
        );
        Ok(OrgOverlayDocumentProjection::new(
            source,
            digest,
            source.lines().any(|line| line.starts_with("* ")),
        ))
    }
}

struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn range(&mut self, upper: usize) -> usize {
        (self.next_u64() as usize) % upper
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RefPointer {
    generation_digest: SearchGenerationDigest,
    artifact_hash: SearchArtifactHash,
    body: Vec<u8>,
    revision: u64,
}

#[derive(Default)]
struct ReferenceModel {
    pointers: BTreeMap<(String, String), RefPointer>,
    edges: BTreeSet<(String, String, String)>,
}

fn project_id(idx: usize) -> SearchProjectId {
    SearchProjectId::new(format!("proj-{idx}")).expect("valid project id")
}

fn workspace_id(idx: usize) -> SearchWorkspaceId {
    SearchWorkspaceId::new(format!("ws-{idx}")).expect("valid workspace id")
}

fn generation_digest(idx: usize) -> SearchGenerationDigest {
    let ch = ["a", "b", "c", "d"][idx % 4];
    SearchGenerationDigest::new(format!("blake3-256:{}", ch.repeat(64)))
        .expect("valid generation digest")
}

fn run_seed(seed: u64) {
    let mut rng = XorShift64::new(seed);
    let engine = MarlinConvergedSearchEngine::new(
        Arc::new(DeterministicOrgPort),
        Arc::new(InMemorySearchStoragePort::new()),
    );
    let mut model = ReferenceModel::default();
    let default_proj = project_id(0);

    let rank_candidates = vec![
        SourceIndexRankCandidate {
            ordinal: 0,
            path: "crates/agent-semantic-search/src/marlin_engine.rs".to_owned(),
            query_keys: vec!["MarlinConvergedSearchEngine".to_owned()],
        },
        SourceIndexRankCandidate {
            ordinal: 1,
            path: "crates/agent-semantic-search/src/source_index_rank.rs".to_owned(),
            query_keys: vec!["rank_source_index_report".to_owned()],
        },
        SourceIndexRankCandidate {
            ordinal: 2,
            path: "crates/agent-semantic-search/src/sorted_record_table.rs".to_owned(),
            query_keys: vec!["SortedRecordTable".to_owned()],
        },
    ];

    for step in 0..60_usize {
        match rng.range(7) {
            // 0: Valid CAS publication
            0 => {
                let ws = workspace_id(rng.range(3));
                let gen_digest = generation_digest(rng.range(4));
                let body = format!("seed={seed}:step={step}:val={}", rng.next_u64()).into_bytes();
                let key = (default_proj.as_str().to_owned(), ws.as_str().to_owned());
                let expected = model.pointers.get(&key).map(|p| p.artifact_hash.clone());
                let prev_rev = model.pointers.get(&key).map_or(0, |p| p.revision);

                let record = engine
                    .publish_generation_artifact(
                        default_proj.clone(),
                        ws,
                        expected.as_ref(),
                        gen_digest.clone(),
                        "application/octet-stream",
                        body.clone(),
                    )
                    .expect("valid CAS publication");

                assert_eq!(record.revision, prev_rev + 1);
                assert_eq!(record.generation_digest, gen_digest);
                model.pointers.insert(
                    key,
                    RefPointer {
                        generation_digest: gen_digest,
                        artifact_hash: record.artifact_hash,
                        body,
                        revision: record.revision,
                    },
                );
            }
            // 1: Stale CAS conflict must fail closed without altering active state
            1 => {
                let ws = workspace_id(rng.range(3));
                let gen_digest = generation_digest(rng.range(4));
                let key = (default_proj.as_str().to_owned(), ws.as_str().to_owned());
                let current = model.pointers.get(&key).cloned();
                let wrong_expected = match &current {
                    Some(_) => None,
                    None => Some(
                        SearchArtifactHash::new(format!("blake3-256:{:064x}", seed + 1))
                            .expect("synthetic hash"),
                    ),
                };

                let err = engine
                    .publish_generation_artifact(
                        default_proj.clone(),
                        ws.clone(),
                        wrong_expected.as_ref(),
                        gen_digest,
                        "application/octet-stream",
                        b"stale-writer".to_vec(),
                    )
                    .expect_err("stale CAS must return PointerConflict");
                assert!(matches!(
                    err,
                    SearchStoragePortError::PointerConflict { .. }
                ));

                let active = engine
                    .load_active_generation_artifact(&default_proj, &ws)
                    .expect("load active after stale conflict");
                assert_eq!(
                    active.map(|(ptr, blob)| (ptr.revision, ptr.artifact_hash, blob.body)),
                    current.map(|p| (p.revision, p.artifact_hash, p.body))
                );
            }
            // 2: Load active pointer & verify BLAKE3 integrity
            2 => {
                let ws = workspace_id(rng.range(3));
                let key = (default_proj.as_str().to_owned(), ws.as_str().to_owned());
                let active = engine
                    .load_active_generation_artifact(&default_proj, &ws)
                    .expect("load active");
                match model.pointers.get(&key) {
                    Some(expected) => {
                        let (ptr, blob) = active.expect("active pointer present");
                        blob.verify_digest().expect("BLAKE3 digest intact");
                        assert_eq!(ptr.revision, expected.revision);
                        assert_eq!(ptr.generation_digest, expected.generation_digest);
                        assert_eq!(ptr.artifact_hash, expected.artifact_hash);
                        assert_eq!(blob.body, expected.body);
                    }
                    None => assert!(active.is_none()),
                }
            }
            // 3: Idempotent topology edge append
            3 => {
                let proj = project_id(rng.range(2));
                let gen_digest = generation_digest(rng.range(4));
                let eid =
                    SearchTopologyEdgeId::new(format!("edge-{}", rng.range(6))).expect("edge id");
                let edge = SearchTopologyEdgeRecord {
                    project_id: proj.clone(),
                    generation_digest: gen_digest.clone(),
                    edge_id: eid.clone(),
                    from_node: "node:a".to_owned(),
                    to_node: "node:b".to_owned(),
                    relation: "depends-on".to_owned(),
                    payload: b"edge".to_vec(),
                };
                let inserted = engine
                    .persist_topology_edges(std::slice::from_ref(&edge))
                    .expect("persist edge");
                let was_new = model.edges.insert((
                    proj.as_str().to_owned(),
                    gen_digest.as_str().to_owned(),
                    eid.as_str().to_owned(),
                ));
                assert_eq!(inserted, usize::from(was_new));
            }
            // 4: Generation-scoped topology edge query isolation
            4 => {
                let proj = project_id(rng.range(2));
                let gen_digest = generation_digest(rng.range(4));
                let loaded = engine
                    .load_topology_edges(&proj, &gen_digest)
                    .expect("load topology edges");
                let expected_ids: BTreeSet<String> = model
                    .edges
                    .iter()
                    .filter(|(p, g, _)| p == proj.as_str() && g == gen_digest.as_str())
                    .map(|(_, _, id)| id.clone())
                    .collect();
                let observed_ids: BTreeSet<String> = loaded
                    .iter()
                    .map(|e| e.edge_id.as_str().to_owned())
                    .collect();
                assert_eq!(observed_ids, expected_ids);
            }
            // 5: Playbook admission gate
            5 => {
                let with_conflict = rng.range(2) == 0;
                let rg_args = if with_conflict {
                    vec!["-n".to_owned(), "query".to_owned(), "crates/".to_owned()]
                } else {
                    vec!["-n".to_owned(), "MarlinConvergedSearchEngine".to_owned()]
                };
                let plan = SearchPlaybookPlanInput {
                    workspace: Some("asp-pbt".to_owned()),
                    languages: vec!["rust".to_owned()],
                    documents: vec!["org".to_owned()],
                    composition: SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(rg_args)),
                };
                let result = engine.admit_search_plan(plan);
                assert_eq!(result.is_err(), with_conflict);
            }
            // 6: Deterministic & monotonic source-index ranking
            _ => {
                let query =
                    ["marlin_engine.rs", "source_index_rank", "SortedRecordTable"][rng.range(3)];
                let request = SourceIndexRankRequest {
                    query: query.to_owned(),
                    candidates: rank_candidates.clone(),
                };
                let first = engine.rank_source_index(request.clone());
                let second = engine.rank_source_index(request);
                assert_eq!(first, second);
                for window in first.ranked_candidates.windows(2) {
                    assert!(window[0].score.total >= window[1].score.total);
                }
            }
        }
    }
}

#[test]
fn stateful_reference_model_holds_across_12000_randomized_transitions() {
    for seed in 1..=200_u64 {
        run_seed(seed);
    }
}
