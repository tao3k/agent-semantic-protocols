// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_topology::{AgentOrgTopologyOverlay, AgentTopologyRelationship};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn admitted() -> AgentOrgTopologyOverlay {
    AgentOrgTopologyOverlay::admit(
        digest('1'),
        digest('2'),
        "src/lib.rs#item=run",
        digest('3'),
        digest('4'),
        "run dispatches the admitted request to the runtime.",
        "* Agent summary\n** Relationship\nrun dispatches runtime request.\n",
        vec![AgentTopologyRelationship {
            from_selector: "src/lib.rs#item=run".to_owned(),
            relation: "dispatches-to".to_owned(),
            to_selector: "src/runtime.rs#item=dispatch".to_owned(),
        }],
    )
    .expect("admitted overlay")
}

#[test]
fn agent_summary_org_ast_and_overlay_have_independent_identities() {
    let overlay = admitted();
    assert_ne!(overlay.summary_digest, overlay.org_source_digest);
    assert_ne!(overlay.org_source_digest, overlay.org_ast_digest);
    assert_ne!(overlay.org_ast_digest, overlay.overlay_digest);
    assert_eq!(overlay.base_topology_generation_digest, digest('2'));
    assert_eq!(overlay.terminal.state, "admitted");
}

#[test]
fn stale_generation_and_unscoped_relationships_fail_closed() {
    let invalid_digest = AgentOrgTopologyOverlay::admit(
        "stale",
        digest('2'),
        "src/lib.rs#item=run",
        digest('3'),
        digest('4'),
        "summary",
        "* Summary\n",
        vec![AgentTopologyRelationship {
            from_selector: "src/a.rs#item=a".to_owned(),
            relation: "calls".to_owned(),
            to_selector: "src/b.rs#item=b".to_owned(),
        }],
    )
    .expect_err("invalid content generation must fail");
    assert_eq!(
        invalid_digest.reason_kind,
        "agent-org-topology-overlay-digest-invalid"
    );

    let unscoped = AgentOrgTopologyOverlay::admit(
        digest('1'),
        digest('2'),
        "src/lib.rs#item=run",
        digest('3'),
        digest('4'),
        "summary",
        "* Summary\n",
        vec![AgentTopologyRelationship {
            from_selector: "src/a.rs#item=a".to_owned(),
            relation: "calls".to_owned(),
            to_selector: "src/b.rs#item=b".to_owned(),
        }],
    )
    .expect_err("relationship outside the selected scope must fail");
    assert_eq!(
        unscoped.reason_kind,
        "agent-org-topology-overlay-relationship-invalid"
    );
}

#[test]
fn relationship_input_order_does_not_change_overlay_identity() {
    let base = admitted();
    let extra = AgentTopologyRelationship {
        from_selector: "src/lib.rs#item=run".to_owned(),
        relation: "reads-from".to_owned(),
        to_selector: "src/config.rs#item=Config".to_owned(),
    };
    let build = |relationships| {
        AgentOrgTopologyOverlay::admit(
            base.source_generation_digest.clone(),
            base.base_topology_generation_digest.clone(),
            base.selector.clone(),
            base.agent_identity_digest.clone(),
            base.prompt_digest.clone(),
            base.summary.clone(),
            base.org_source.clone(),
            relationships,
        )
        .expect("overlay")
    };
    let left = build(vec![base.relationships[0].clone(), extra.clone()]);
    let right = build(vec![extra, base.relationships[0].clone()]);
    assert_eq!(left.relationships, right.relationships);
    assert_eq!(left.overlay_digest, right.overlay_digest);
}

#[test]
fn serialized_overlay_cannot_self_attest_after_org_mutation() {
    let mut overlay = admitted();
    overlay.org_source.push_str("mutated\n");
    let error = overlay
        .validate()
        .expect_err("stale Org identity must fail");
    assert_eq!(
        error.reason_kind,
        "agent-org-topology-overlay-derived-identity-mismatch"
    );
}

#[test]
fn overlay_joins_only_its_exact_base_search_topology() {
    let overlay = admitted();
    overlay
        .validate_against_base(&digest('1'), &digest('2'))
        .expect("matching base topology");
    let error = overlay
        .validate_against_base(&digest('1'), &digest('9'))
        .expect_err("another base topology must fail");
    assert_eq!(
        error.reason_kind,
        "agent-org-topology-overlay-base-binding-mismatch"
    );
}
