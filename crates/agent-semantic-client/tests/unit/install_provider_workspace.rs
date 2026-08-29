use super::{
    ProviderWorkspaceInstallDescriptor, WorkspaceLaunchDescriptor,
    WorkspaceRuntimeDependencyDescriptor, artifact_snapshot, copy_artifact_root,
    materialize_runtime_dependencies, resolve_runtime_dependencies,
};
use std::path::{Path, PathBuf};

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

#[test]
fn document_provider_workspace_descriptors_have_independent_schema_receipts() {
    for (source, language_id, provider_id, binary) in [
        (
            include_str!(
                "../../../../languages/orgize/provider/asp-org-provider-workspace-install.json"
            ),
            "org",
            "asp-org",
            "asp-org",
        ),
        (
            include_str!(
                "../../../../languages/orgize/provider/asp-md-provider-workspace-install.json"
            ),
            "md",
            "asp-md",
            "asp-md",
        ),
    ] {
        let descriptor: ProviderWorkspaceInstallDescriptor =
            serde_json::from_str(source).expect("decode document provider workspace descriptor");
        descriptor
            .validate()
            .expect("validate document provider workspace descriptor");
        descriptor
            .validate_registration_identity(language_id, provider_id, binary)
            .expect("document provider workspace identity");
        assert!(descriptor.schema_bundle_receipt.starts_with(language_id));
    }
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
