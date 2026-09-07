// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_topology::{PROJECT_TOPOLOGY_MANIFEST_PATH, ProjectTopologyManifest};
use std::fs;

fn manifest(properties: &str) -> String {
    format!(
        r#"#+TITLE: Project Topology Program Manifest v1
:PROPERTIES:
:CONTRACT_ORG: [[../../../org/contracts/project.topology-program.v1.org][project.topology-program.v1]]
:END:

* Project Topology Program
:PROPERTIES:
{properties}
:END:
"#,
    )
}

fn valid_properties() -> &'static str {
    r#":TOPOLOGY_PROGRAM_ID: project-topology-program-example
:PROJECT_WORKSPACE_IDENTITY: git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root
:WORKSPACE_ROOT_PATH: .
:PORTABILITY: cross-machine
:REPOSITORY_ALIASES: []
:SOURCE_SNAPSHOT_DIGEST: blake3-256:1111111111111111111111111111111111111111111111111111111111111111
:MRR_PRELUDE_DIGEST: blake3-256:4444444444444444444444444444444444444444444444444444444444444444
:PROJECT_PROGRAM_DIGEST: blake3-256:5555555555555555555555555555555555555555555555555555555555555555
:COMPILED_PROGRAM_ABI_DIGEST: blake3-256:7777777777777777777777777777777777777777777777777777777777777777
:MRR_BUNDLE_DIGEST: blake3-256:8888888888888888888888888888888888888888888888888888888888888888
:TOPOLOGY_ROOT_DIGEST: blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"#
}

#[test]
fn manifest_projects_one_parser_owned_project_workspace_binding() {
    assert_eq!(
        PROJECT_TOPOLOGY_MANIFEST_PATH,
        ".agents/asp/topology/manifest.org"
    );
    let manifest = ProjectTopologyManifest::parse_org(&manifest(valid_properties()))
        .expect("canonical manifest");
    let binding = manifest.project_workspace();
    assert_eq!(
        binding.project_workspace_identity(),
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root"
    );
    assert_eq!(binding.workspace_root_path(), ".");
    assert_eq!(binding.portability(), "cross-machine");
    assert!(binding.repository_aliases().is_empty());
}

#[test]
fn manifest_rejects_duplicate_level_one_program_declarations() {
    let source = format!(
        "{}\n* Duplicate\n:PROPERTIES:\n{}\n:END:\n",
        manifest(valid_properties()),
        valid_properties()
    );
    let error = ProjectTopologyManifest::parse_org(&source)
        .expect_err("document order cannot choose an authority");
    assert_eq!(error.reason_kind(), "topology-manifest-ambiguous");
}

#[test]
fn manifest_rejects_runtime_generated_workspace_identity() {
    let properties = valid_properties().replace(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root",
        "workspace-23cc5ba784c605ae",
    );
    let error = ProjectTopologyManifest::parse_org(&manifest(&properties))
        .expect_err("Runtime identity cannot replace GitOps identity");
    assert_eq!(
        error.reason_kind(),
        "topology-project-workspace-identity-invalid"
    );
}

#[test]
fn manifest_rejects_absolute_workspace_root() {
    let properties = valid_properties().replace(
        ":WORKSPACE_ROOT_PATH: .",
        ":WORKSPACE_ROOT_PATH: /tmp/project",
    );
    let error = ProjectTopologyManifest::parse_org(&manifest(&properties))
        .expect_err("Host checkout placement is not a repository binding");
    assert_eq!(error.reason_kind(), "topology-workspace-root-path-invalid");
}

#[test]
fn manifest_rejects_non_array_repository_aliases() {
    let properties =
        valid_properties().replace(":REPOSITORY_ALIASES: []", ":REPOSITORY_ALIASES: none");
    let error = ProjectTopologyManifest::parse_org(&manifest(&properties))
        .expect_err("aliases use one compact typed representation");
    assert_eq!(error.reason_kind(), "topology-manifest-aliases-invalid");
}

#[test]
fn manifest_rejects_noncanonical_repository_alias_order() {
    let properties = valid_properties().replace(
        ":REPOSITORY_ALIASES: []",
        r#":REPOSITORY_ALIASES: ["git+https://example.dev/z.git","git+https://example.dev/a.git"]"#,
    );
    let error = ProjectTopologyManifest::parse_org(&manifest(&properties))
        .expect_err("alias ordering is part of deterministic manifest projection");
    assert_eq!(error.reason_kind(), "topology-manifest-aliases-invalid");
}

#[test]
fn manifest_requires_the_org_contract_binding() {
    let source = manifest(valid_properties()).replace(
        "[[../../../org/contracts/project.topology-program.v1.org][project.topology-program.v1]]",
        "[[../../../org/contracts/other.v1.org][other.v1]]",
    );
    let error = ProjectTopologyManifest::parse_org(&source)
        .expect_err("another Org contract cannot authorize this manifest");
    assert_eq!(error.reason_kind(), "topology-manifest-contract-mismatch");
}

#[test]
fn canonical_loader_reads_only_the_git_tracked_manifest_path() {
    let project = tempfile::tempdir().expect("temporary project");
    let manifest_path = project.path().join(PROJECT_TOPOLOGY_MANIFEST_PATH);
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("topology directory");
    fs::write(&manifest_path, manifest(valid_properties())).expect("manifest fixture");

    let manifest = ProjectTopologyManifest::load_from_project_root(project.path())
        .expect("canonical manifest path");
    assert_eq!(manifest.project_workspace().workspace_root_path(), ".");
}

#[test]
fn canonical_loader_does_not_fallback_when_manifest_is_absent() {
    let project = tempfile::tempdir().expect("temporary project");
    let error = ProjectTopologyManifest::load_from_project_root(project.path())
        .expect_err("no environment, Git remote, or Runtime fallback is allowed");
    assert_eq!(error.reason_kind(), "topology-manifest-unavailable");
}
