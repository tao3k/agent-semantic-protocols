// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust regression tests proving `agent-semantic-topology` operates
//! completely through `OrgDocumentProjectionPort` with zero embedded Orgize FFI.

use agent_semantic_topology::{
    AgentOrgTopologyOverlay, AgentTopologyRelationship, OrgDocumentProjection,
    OrgDocumentProjectionPort, OrgOverlayDocumentProjection, OrgProjectionPortError,
    OrgPropertyEntry, OrgSectionProjection, ProjectTopologyManifest, ProjectTopologySourceProgram,
};

struct StubMarlinOrgPort;

impl OrgDocumentProjectionPort for StubMarlinOrgPort {
    fn project_document(
        &self,
        source: &str,
    ) -> Result<OrgDocumentProjection, OrgProjectionPortError> {
        if source.contains("TOPOLOGY_PROGRAM_ID") {
            return Ok(OrgDocumentProjection::new(
                OrgDocumentProjection::digest_for_source(source),
                vec![OrgPropertyEntry::new(
                    "CONTRACT_ORG",
                    "[[../../../org/contracts/asp/topology/program.v1.org][asp.topology.program.v1]]",
                )],
                vec![
                    OrgSectionProjection::new(
                        1,
                        None,
                        vec![
                            OrgPropertyEntry::new(
                                "TOPOLOGY_PROGRAM_ID",
                                "mrr.topology.standard.v1",
                            ),
                            OrgPropertyEntry::new("ENGINE_PROFILE", "ascent-seminaive.v1"),
                        ],
                    ),
                    OrgSectionProjection::new(
                        2,
                        Some(0),
                        vec![
                            OrgPropertyEntry::new(
                                "RULE_ID",
                                "mrr.topology.domain.depends-on-config.v1",
                            ),
                            OrgPropertyEntry::new("HEAD_RELATION", "DEPENDS_ON_CONFIG"),
                            OrgPropertyEntry::new("LEFT_RELATION", "CALLS"),
                            OrgPropertyEntry::new("RIGHT_RELATION", "READS_CONFIG"),
                            OrgPropertyEntry::new("JOIN", "left.to=right.from"),
                        ],
                    ),
                ],
            ));
        }

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
fn topology_manifest_and_program_load_via_marlin_org_port_without_orgize() {
    let port = StubMarlinOrgPort;
    let manifest = ProjectTopologyManifest::parse_with_port("* Project Workspace\n", &port)
        .expect("manifest via Marlin Org port");
    assert_eq!(
        manifest.project_workspace().project_workspace_identity(),
        "git+https://github.com/tao3k/marlin-agent-core.git#workspace/root"
    );

    let program = ProjectTopologySourceProgram::parse_with_port(":TOPOLOGY_PROGRAM_ID:", &port)
        .expect("program via Marlin Org port");
    assert_eq!(program.program_id(), "mrr.topology.standard.v1");
    assert_eq!(program.engine_profile(), "ascent-seminaive.v1");
    assert_eq!(program.rules().len(), 1);
    assert!(program.inference_program().is_ok());
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

#[test]
fn program_rejects_unscoped_headline_contract_and_nested_program_lookalike() {
    let unscoped_contract = OrgDocumentProjection::new(
        digest('a'),
        vec![],
        vec![OrgSectionProjection::new(
            1,
            None,
            vec![OrgPropertyEntry::new(
                "CONTRACT_ORG",
                "[[program.v1.org][asp.topology.program.v1]]",
            )],
        )],
    );
    let err = ProjectTopologySourceProgram::from_org_projection(&unscoped_contract)
        .expect_err("headline contract must not authorize document");
    assert_eq!(
        err.reason_kind(),
        "topology-source-program-contract-mismatch"
    );

    let nested_program = OrgDocumentProjection::new(
        digest('b'),
        vec![OrgPropertyEntry::new(
            "CONTRACT_ORG",
            "[[program.v1.org][asp.topology.program.v1]]",
        )],
        vec![
            OrgSectionProjection::new(
                1,
                None,
                vec![
                    OrgPropertyEntry::new("TOPOLOGY_PROGRAM_ID", "mrr.topology.standard.v1"),
                    OrgPropertyEntry::new("ENGINE_PROFILE", "ascent-seminaive.v1"),
                ],
            ),
            OrgSectionProjection::new(
                2,
                Some(0),
                vec![OrgPropertyEntry::new(
                    "TOPOLOGY_PROGRAM_ID",
                    "not-a-top-level-program",
                )],
            ),
            OrgSectionProjection::new(
                2,
                Some(0),
                vec![
                    OrgPropertyEntry::new("RULE_ID", "mrr.topology.domain.depends-on-config.v1"),
                    OrgPropertyEntry::new("HEAD_RELATION", "DEPENDS_ON_CONFIG"),
                    OrgPropertyEntry::new("LEFT_RELATION", "CALLS"),
                    OrgPropertyEntry::new("RIGHT_RELATION", "READS_CONFIG"),
                    OrgPropertyEntry::new("JOIN", "left.to=right.from"),
                ],
            ),
        ],
    );
    let program = ProjectTopologySourceProgram::from_org_projection(&nested_program)
        .expect("nested lookalike must be ignored");
    assert_eq!(program.program_id(), "mrr.topology.standard.v1");
}
