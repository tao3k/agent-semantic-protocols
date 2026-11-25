// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust regression tests proving `agent-semantic-topology` operates
//! completely through `OrgDocumentProjectionPort` with zero embedded Orgize FFI.

use agent_semantic_topology::{
    AgentOrgTopologyOverlay, AgentTopologyRelationship, OrgDocumentProjection,
    OrgDocumentProjectionPort, OrgOverlayDocumentProjection, OrgProjectionPortError,
    OrgPropertyEntry, OrgSectionProjection, ProjectTopologyManifest,
};

struct StubMarlinOrgPort;

impl OrgDocumentProjectionPort for StubMarlinOrgPort {
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
                    OrgPropertyEntry::new("PROJECT_WORKSPACE_ID", "marlin-converged"),
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
        let has_headline = source.lines().any(|line| line.starts_with("* "));
        let ast_digest = format!(
            "blake3-256:{}",
            blake3::hash(format!("marlin-fold-ast:{source}").as_bytes()).to_hex()
        );
        Ok(OrgOverlayDocumentProjection::new(
            source,
            ast_digest,
            has_headline,
        ))
    }
}

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

#[test]
fn topology_manifest_loads_via_marlin_org_port_without_orgize() {
    let port = StubMarlinOrgPort;
    let manifest = ProjectTopologyManifest::parse_with_port("* Project Workspace\n", &port)
        .expect("manifest via Marlin Org port");
    assert_eq!(
        manifest.project_workspace().project_workspace_identity(),
        "git+https://github.com/tao3k/marlin-agent-core.git#workspace/root"
    );
}

#[test]
fn agent_org_overlay_admits_and_validates_via_marlin_org_port_without_orgize() {
    let port = StubMarlinOrgPort;
    let overlay = AgentOrgTopologyOverlay::admit_with_port(
        digest('1'),
        digest('2'),
        "src/lib.rs#item=run",
        digest('3'),
        digest('4'),
        digest('5'),
        "run dispatches the admitted request to the runtime.",
        "* Agent summary\n** Relationship\nrun dispatches runtime request.\n",
        vec![AgentTopologyRelationship {
            from_selector: "src/lib.rs#item=run".to_owned(),
            relation: "dispatches-to".to_owned(),
            to_selector: "src/runtime.rs#item=dispatch".to_owned(),
        }],
        &port,
    )
    .expect("overlay via Marlin Org port");

    overlay
        .validate_against_base_with_port(&digest('1'), &digest('2'), &port)
        .expect("matching base topology");

    let mut mutated = overlay.clone();
    mutated.org_source.push_str("tampered\n");
    let err = mutated
        .validate_with_port(&port)
        .expect_err("tampered overlay must fail closed");
    assert_eq!(
        err.reason_kind,
        "agent-org-topology-overlay-derived-identity-mismatch"
    );
}
