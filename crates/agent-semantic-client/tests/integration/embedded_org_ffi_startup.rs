// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Isolated process qualification for explicit embedded parser startup.

#[test]
fn explicit_startup_precedes_parallel_lossless_org_parsing() {
    // SAFETY: this binary contains one test; no application workers or children
    // are created before the explicit startup window finishes.
    unsafe { orgize::initialize_native_runtime() }.expect("embedded parser startup");
    qualify_topology_projection();
    let source = "* TODO Search closure\nUnicode: 搜索 λ\n";
    std::thread::scope(|scope| {
        let workers = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let document = orgize::Org::try_parse(source).expect("FFI parse");
                    assert_eq!(document.to_org(), source);
                    assert_eq!(document.receipt().language, "org-mode");
                    assert!(document.receipt().parser_digest.is_some());
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("parser worker");
        }
    });
}

fn qualify_topology_projection() {
    use agent_semantic_topology::{
        AgentOrgTopologyOverlay, AgentTopologyRelationship, ProjectTopologyManifest,
    };
    let properties = ":PROJECT_WORKSPACE_ID: asp\n:PROJECT_WORKSPACE_IDENTITY: git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root\n:WORKSPACE_ROOT_PATH: .\n:PORTABILITY: cross-machine\n:REPOSITORY_ALIASES: []";
    let source = format!(
        "#+TITLE: Workspace\n:PROPERTIES:\n:CONTRACT_ORG: [[../../../org/contracts/project.workspace-manifest.v1.org][project.workspace-manifest.v1]]\n:END:\n* Workspace\n:PROPERTIES:\n{properties}\n:END:\n"
    );
    let binding = ProjectTopologyManifest::parse_org(&source).expect("Scheme-owned manifest");
    assert_eq!(binding.project_workspace().workspace_root_path(), ".");
    let nested = format!("{source}** Nested decoy\n:PROPERTIES:\n{properties}\n:END:\n");
    assert_eq!(
        ProjectTopologyManifest::parse_org(&nested).unwrap(),
        binding
    );
    let duplicate = format!("{source}* Duplicate\n:PROPERTIES:\n{properties}\n:END:\n");
    assert_eq!(
        ProjectTopologyManifest::parse_org(&duplicate)
            .unwrap_err()
            .reason_kind(),
        "topology-manifest-ambiguous"
    );
    let duplicate_property = source.replace(
        ":PORTABILITY: cross-machine",
        ":PORTABILITY: cross-machine\n:PORTABILITY: cross-machine",
    );
    assert_eq!(
        ProjectTopologyManifest::parse_org(&duplicate_property)
            .unwrap_err()
            .reason_kind(),
        "topology-manifest-property-duplicate"
    );
    let digest = |byte: char| format!("blake3-256:{}", byte.to_string().repeat(64));
    let mut overlay = AgentOrgTopologyOverlay::admit(
        digest('1'),
        digest('2'),
        "src/lib.rs#item=run",
        digest('3'),
        digest('4'),
        digest('5'),
        "Dispatch the search request.",
        "* Search request\nUnicode: 搜索 λ\n",
        vec![AgentTopologyRelationship {
            from_selector: "src/lib.rs#item=run".into(),
            relation: "dispatches-to".into(),
            to_selector: "src/runtime.rs#item=dispatch".into(),
        }],
    )
    .expect("source-backed overlay");
    overlay.validate().expect("current AST identity");
    overlay.org_source.push_str("Changed source.\n");
    assert!(
        overlay.validate().is_err(),
        "changed source cannot retain its AST identity"
    );
}
