// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! End-to-end verification of `MarlinConvergedSearchEngine` running solely
//! through `OrgDocumentProjectionPort` and `SearchEngineStoragePort` with
//! zero embedded Orgize FFI and zero embedded Turso DB.

use std::sync::Arc;

use agent_semantic_topology::{
    AgentTopologyRelationship, OrgDocumentProjection, OrgDocumentProjectionPort,
    OrgOverlayDocumentProjection, OrgProjectionPortError, OrgPropertyEntry, OrgSectionProjection,
};

use crate::{
    InMemorySearchStoragePort, MarlinConvergedSearchEngine, SearchGenerationDigest,
    SearchProjectId, SearchStoragePortError, SearchTopologyEdgeId, SearchTopologyEdgeRecord,
    SearchWorkspaceId,
};

struct MarlinMockOrgPort;

impl OrgDocumentProjectionPort for MarlinMockOrgPort {
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
                    OrgPropertyEntry::new("PROJECT_WORKSPACE_ID", "marlin-core"),
                    OrgPropertyEntry::new(
                        "PROJECT_WORKSPACE_IDENTITY",
                        "git+https://github.com/tao3k/marlin-agent-core.git#workspace/root",
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
        let ast_digest = format!(
            "blake3-256:{}",
            blake3::hash(format!("marlin-org-fold:{source}").as_bytes()).to_hex()
        );
        Ok(OrgOverlayDocumentProjection::new(
            source,
            ast_digest,
            source.lines().any(|line| line.starts_with("* ")),
        ))
    }
}

fn digest(ch: char) -> String {
    format!("blake3-256:{}", ch.to_string().repeat(64))
}

#[test]
fn converged_marlin_search_engine_drives_org_and_storage_without_orgize_or_turso() {
    let engine = MarlinConvergedSearchEngine::new(
        Arc::new(MarlinMockOrgPort),
        Arc::new(InMemorySearchStoragePort::new()),
    );

    let manifest = engine
        .admit_topology_manifest("* Project Workspace\n")
        .expect("manifest admitted through Marlin Org port");
    assert_eq!(
        manifest.project_workspace().project_workspace_identity(),
        "git+https://github.com/tao3k/marlin-agent-core.git#workspace/root"
    );

    let overlay = engine
        .admit_org_overlay(
            digest('1'),
            digest('2'),
            "src/search.rs#item=execute",
            digest('3'),
            digest('4'),
            digest('5'),
            "execute runs converged search",
            "* Converged Search\nUses Marlin ports.\n",
            vec![AgentTopologyRelationship {
                from_selector: "src/search.rs#item=execute".to_owned(),
                relation: "delegates-to".to_owned(),
                to_selector: "src/storage.rs#item=MarlinStorage".to_owned(),
            }],
        )
        .expect("overlay admitted through Marlin Org port");
    assert_eq!(overlay.terminal.state, "admitted");

    let project_id = SearchProjectId::new("project-marlin").unwrap();
    let workspace_id = SearchWorkspaceId::new("workspace-root").unwrap();
    let gen1 = SearchGenerationDigest::new(digest('a')).unwrap();
    let gen2 = SearchGenerationDigest::new(digest('b')).unwrap();

    let first = engine
        .publish_generation_artifact(
            project_id.clone(),
            workspace_id.clone(),
            None,
            gen1.clone(),
            "application/vnd.asp.search-generation+json",
            br#"{"generation":"v1"}"#.to_vec(),
        )
        .expect("initial generation publish");
    assert_eq!(first.revision, 1);

    let conflict = engine
        .publish_generation_artifact(
            project_id.clone(),
            workspace_id.clone(),
            None,
            gen2.clone(),
            "application/vnd.asp.search-generation+json",
            br#"{"generation":"v2"}"#.to_vec(),
        )
        .expect_err("stale CAS publish must fail closed");
    assert!(matches!(
        conflict,
        SearchStoragePortError::PointerConflict { .. }
    ));

    let second = engine
        .publish_generation_artifact(
            project_id.clone(),
            workspace_id.clone(),
            Some(&first.artifact_hash),
            gen2.clone(),
            "application/vnd.asp.search-generation+json",
            br#"{"generation":"v2"}"#.to_vec(),
        )
        .expect("CAS generation advance");
    assert_eq!(second.revision, 2);

    let (loaded_ptr, loaded_blob) = engine
        .load_active_generation_artifact(&project_id, &workspace_id)
        .expect("load active generation")
        .expect("active generation exists");
    assert_eq!(loaded_ptr, second);
    assert_eq!(loaded_blob.body, br#"{"generation":"v2"}"#);

    let inserted = engine
        .persist_topology_edges(&[SearchTopologyEdgeRecord {
            project_id: project_id.clone(),
            generation_digest: gen2.clone(),
            edge_id: SearchTopologyEdgeId::new("edge-1").unwrap(),
            from_node: "src/search.rs#item=execute".to_owned(),
            to_node: "src/storage.rs#item=MarlinStorage".to_owned(),
            relation: "DELEGATES_TO".to_owned(),
            payload: br#"{"source":"marlin-converged"}"#.to_vec(),
        }])
        .expect("persist topology edges");
    assert_eq!(inserted, 1);

    let loaded_edges = engine
        .load_topology_edges(&project_id, &gen2)
        .expect("list topology edges");
    assert_eq!(loaded_edges.len(), 1);
    assert_eq!(loaded_edges[0].relation, "DELEGATES_TO");
}
