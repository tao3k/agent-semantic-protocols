// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared pure-Rust and optional embedded-Orgize helpers for topology unit tests.

use agent_semantic_topology::{
    OrgDocumentProjection, OrgPropertyEntry, OrgSectionProjection, ProjectTopologyManifest,
};

pub(crate) fn canonical_test_manifest() -> ProjectTopologyManifest {
    let projection = OrgDocumentProjection::new(
        OrgDocumentProjection::digest_for_source("project.workspace-manifest.v1"),
        vec![OrgPropertyEntry::new(
            "CONTRACT_ORG",
            "[[../contracts/project.workspace-manifest.v1.org][project.workspace-manifest.v1]]",
        )],
        vec![OrgSectionProjection::new(
            1,
            None,
            vec![
                OrgPropertyEntry::new("PROJECT_WORKSPACE_ID", "agent-semantic-protocols-root"),
                OrgPropertyEntry::new(
                    "PROJECT_WORKSPACE_IDENTITY",
                    "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
                ),
                OrgPropertyEntry::new("WORKSPACE_ROOT_PATH", "."),
                OrgPropertyEntry::new("PORTABILITY", "cross-machine"),
                OrgPropertyEntry::new("REPOSITORY_ALIASES", "[]"),
            ],
        )],
    );
    ProjectTopologyManifest::from_org_projection(&projection)
        .expect("canonical test Project Topology manifest")
}

#[cfg(feature = "embedded-orgize")]
pub(crate) fn ensure_orgize_native_runtime() {
    // SAFETY: unit tests in this crate are pure in-memory checks that do not
    // spawn subprocesses or alter host stdio/signal state; `initialize_native_runtime`
    // is process-global and idempotent via `OnceLock`.
    unsafe { orgize::initialize_native_runtime() }.expect("native Org parser startup");
}
