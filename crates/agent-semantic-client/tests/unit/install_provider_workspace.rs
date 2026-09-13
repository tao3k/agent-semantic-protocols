// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ProviderWorkspaceInstallDescriptor;
use super::WorkspaceLaunchDescriptor;
use super::WorkspaceRuntimeDependencyDescriptor;
use super::artifact_snapshot;
use super::copy_artifact_root;
use super::materialize_runtime_dependencies;
use super::resolve_runtime_dependencies;
use super::validate_embedded_provider_registration;
use std::path::Path;
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "asp-provider-workspace-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("fixture clock")
                .as_nanos()
        ));
        std::fs::create_dir(&root).expect("create fixture root");
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("fixture file parent"))
        .expect("create fixture file parent");
    std::fs::write(path, bytes).expect("write fixture file");
}

#[test]
fn provider_workspace_registration_must_equal_the_client_build_contract() {
    let embedded = agent_semantic_provider_protocol::builtin_provider_registrations()
        .expect("embedded registrations")
        .into_iter()
        .find(|registration| registration.provider_id == "asp-rust")
        .expect("Rust registration");
    validate_embedded_provider_registration(&embedded).expect("exact embedded registration");
    let capability = &embedded.registration["searchCapabilities"]["enhancedSyntaxQueryCapability"];
    assert_eq!(
        capability["schemaId"],
        "agent.semantic-protocols.enhanced-tree-sitter-query-capability-table"
    );
    assert!(capability.get("$ref").is_none());

    let mut drifted = embedded;
    drifted.registration["namespace"] = serde_json::json!("forged-rust");
    let error = validate_embedded_provider_registration(&drifted)
        .expect_err("workspace registration drift requires rebuilding the client");
    assert!(error.contains("provider-registration-requires-client-rebuild"));
}

#[test]
fn workspace_install_v1_accepts_the_canonical_language_identity() {
    let descriptor: ProviderWorkspaceInstallDescriptor = serde_json::from_str(
        r#"{
          "$schema":"../schemas/provider-workspace-install.schema.json",
          "schemaId":"agent.semantic-protocols.provider-workspace-install",
          "schemaVersion":"1",
          "schemaAuthority":"https://tao3k.github.io/agent-semantic-protocols/schemas/",
          "languageId":"julia",
          "providerId":"asp-julia",
          "binary":"asp-julia",
          "providerRegistration":"asp-provider-registration.json",
          "schemaBundleReceipt":"../schemas/.asp-schema-manager-receipt.json",
          "workspaceArtifact":{"root":"build/provider","entrypoint":"."},
          "workspaceBuild":{
            "program":"build.sh",
            "args":[],
            "workingDirectory":".",
            "sourceSnapshotAnchors":["Project.toml"],
            "derivedPaths":["build"],
            "env":{}
          }
        }"#,
    )
    .expect("decode canonical v1 workspace install descriptor");

    assert_eq!(descriptor.language_id, "julia");
    assert_eq!(descriptor.provider_id, "asp-julia");
}

#[cfg(unix)]
#[test]
fn external_launch_runtime_dependency_is_materialized_into_the_immutable_artifact() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("runtime-dependency");
    let launch_prefix = fixture.root.join("python-prefix");
    let launch_program = launch_prefix.join("bin/python3");
    let runtime_library = launch_prefix.join("lib/libpython3.13.dylib");
    write(&launch_program, b"python-interpreter");
    write(&runtime_library, b"python-runtime-library");

    let source_root = fixture.root.join("venv");
    std::fs::create_dir_all(source_root.join("bin")).expect("create venv bin");
    write(&source_root.join("bin/asp-python"), b"provider-entrypoint");
    symlink(&launch_program, source_root.join("bin/python3")).expect("link venv python");

    let dependencies = resolve_runtime_dependencies(
        &source_root,
        Some(&WorkspaceLaunchDescriptor {
            program: "bin/python3".to_owned(),
            args: vec!["bin/asp-python".to_owned()],
            program_relative_to_artifact: true,
            args_relative_to_artifact: true,
        }),
        &[WorkspaceRuntimeDependencyDescriptor {
            source: "lib/libpython3.13.dylib".to_owned(),
            target: "lib/libpython3.13.dylib".to_owned(),
        }],
    )
    .expect("resolve runtime dependency from canonical launch prefix");
    assert_eq!(
        dependencies[0].source,
        runtime_library
            .canonicalize()
            .expect("canonical runtime library")
    );

    let staged_root = fixture.root.join("stage/root");
    std::fs::create_dir(staged_root.parent().expect("staged root parent"))
        .expect("create artifact stage");
    copy_artifact_root(&source_root, &staged_root).expect("copy artifact root");
    assert!(!staged_root.join("lib/libpython3.13.dylib").exists());
    materialize_runtime_dependencies(&staged_root, &dependencies)
        .expect("materialize runtime dependency");
    assert_eq!(
        std::fs::read(staged_root.join("lib/libpython3.13.dylib"))
            .expect("read staged runtime library"),
        b"python-runtime-library"
    );
    let (_, leaf_count) = artifact_snapshot(&staged_root).expect("snapshot staged artifact");
    assert_eq!(leaf_count, 3);
}
