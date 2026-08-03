use agent_semantic_hook::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, builtin_provider_manifests, provider_manifest_digest,
};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn write_rust_activation(root: &Path) -> std::path::PathBuf {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("rust manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let installed_provider = ensure_state_home_v1_provider(root, manifest.binary());
    write_project_resolution_provider(
        &installed_provider,
        "rust",
        "rs-harness",
        ".rs",
        &["."],
        &["vendor"],
    );
    let resolved_execution_prefix = vec![installed_provider.display().to_string()];
    let verified_executable_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed_provider)
            .expect("provider executable artifact digest");
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &resolved_execution_prefix,
        &verified_executable_artifact_digest,
    )
    .expect("provider execution command digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = agent_semantic_hook::HookActivation {
        rankers: Vec::new(),
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: activation_project_root.display().to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id().to_owned(),
            manifest_digest,
            language_id: manifest.language_id().clone(),
            provider_id: manifest.provider_id().clone(),
            binary: manifest.binary().to_owned(),
            execution: manifest.execution(),
            provider_command_prefix: Vec::new(),
            execution_command_digest,
            search_capabilities: manifest.search_capabilities().clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
            query_pack_descriptor: manifest.query_pack_descriptor().clone(),
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                config_files: vec![
                    "Cargo.toml".to_string(),
                    "crates/app/Cargo.toml".to_string(),
                    "vendor/tool/Cargo.toml".to_string(),
                ],
                source_extensions: vec!["rs".to_string()],
            },
        }],
    };
    agent_semantic_hook::write_activation(&activation_path, &activation).expect("write activation");
    activation_path
}

pub(super) fn write_gerbil_activation_with_project_resolution(
    root: &Path,
    provider_bin: &Path,
    source_roots: &[&str],
) -> std::path::PathBuf {
    write_gerbil_activation_with_command_prefix(
        root,
        vec![provider_bin.display().to_string()],
        source_roots,
    )
}

pub(crate) fn write_gerbil_activation_with_command_prefix(
    root: &Path,
    provider_command_prefix: Vec<String>,
    source_roots: &[&str],
) -> std::path::PathBuf {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "gerbil-scheme")
        .expect("gerbil manifest");
    let manifest_digest = provider_manifest_digest(&manifest).expect("manifest digest");
    let semantic_registry_digest = agent_semantic_hook::semantic_registry_digest();
    let installed_provider = provider_command_prefix
        .first()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| ensure_state_home_v1_provider(root, manifest.binary()));
    if provider_command_prefix.is_empty() {
        write_project_resolution_provider(
            &installed_provider,
            "gerbil-scheme",
            "gerbil-scheme-harness",
            ".ss",
            source_roots,
            &[],
        );
    }
    let resolved_execution_prefix = vec![installed_provider.display().to_string()];
    let verified_executable_artifact_digest =
        agent_semantic_content_identity::file_content_digest_v1(&installed_provider)
            .expect("provider executable artifact digest");
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &resolved_execution_prefix,
        &verified_executable_artifact_digest,
    )
    .expect("provider execution command digest");
    let routes =
        agent_semantic_hook::materialize_provider_routes(&manifest).expect("provider routes");
    let activation_project_root =
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let activation_path = root.join(".cache/agent-semantic-protocol/hooks/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation parent");
    let activation = agent_semantic_hook::HookActivation {
        rankers: Vec::new(),
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: agent_semantic_hook::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: activation_project_root.display().to_string(),
        generated_by: agent_semantic_hook::ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "test".to_string(),
        },
        generated_at: None,
        providers: vec![agent_semantic_hook::ActivatedProviderConfig {
            manifest_id: manifest.manifest_id().to_owned(),
            manifest_digest,
            language_id: manifest.language_id().clone(),
            provider_id: manifest.provider_id().clone(),
            binary: manifest.binary().to_owned(),
            execution: manifest.execution(),
            provider_command_prefix: Vec::new(),
            execution_command_digest,
            search_capabilities: manifest.search_capabilities().clone(),
            semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
            query_pack_descriptor: manifest.query_pack_descriptor().clone(),
            semantic_registry_digest,
            routes,
            coverage: agent_semantic_hook::ActivationCoverage {
                package_roots: vec![".".to_string()],
                config_files: vec!["gerbil.pkg".to_string()],
                source_extensions: vec!["ss".to_string()],
            },
        }],
    };
    agent_semantic_hook::write_activation(&activation_path, &activation).expect("write activation");
    activation_path
}

pub(super) fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set executable");
    }
}

pub(super) fn noop_provider_command_prefix() -> Vec<String> {
    Vec::new()
}

fn ensure_state_home_v1_provider(root: &Path, binary: &str) -> std::path::PathBuf {
    let provider_bin = home_local_provider_path(root, binary);
    if !provider_bin.exists() {
        std::fs::create_dir_all(provider_bin.parent().expect("provider parent"))
            .expect("create state home provider bin");
        std::fs::write(&provider_bin, "#!/bin/sh\nexit 2\n")
            .expect("write state home provider fixture");
        make_executable(&provider_bin);
    }
    provider_bin
}

pub(crate) fn write_project_resolution_provider(
    provider_bin: &Path,
    language_id: &str,
    provider_id: &str,
    extension: &str,
    source_roots: &[&str],
    excluded_roots: &[&str],
) {
    std::fs::create_dir_all(provider_bin.parent().expect("provider parent"))
        .expect("create typed project-resolution provider parent");
    let project_entry = match language_id {
        "rust" => "Cargo.toml",
        "python" => "pyproject.toml",
        "gerbil-scheme" => "gerbil.pkg",
        _ => "Project.toml",
    };
    let parser_id = match language_id {
        "rust" => "rust.cargo-toml",
        "python" => "python.pyproject-toml",
        "gerbil-scheme" => "gerbil.package-spec",
        _ => "julia.pkg-project-toml",
    };
    let project_resolution_inputs = serde_json::json!({
        "languageId": language_id,
        "providerId": provider_id,
        "extension": extension,
        "sourceRoots": source_roots,
        "excludedRoots": excluded_roots,
        "projectEntry": project_entry,
        "parserId": parser_id,
    });
    let configuration = project_resolution_inputs.to_string();
    let script = format!(
        r#"#!/usr/bin/env -S python3 -S
import json
import re
import sys

CONFIG = json.loads({configuration:?})

if len(sys.argv) != 2:
    raise SystemExit(2)

if sys.argv[1] == "projection-batch-stdin":
    frame = sys.stdin.buffer.read()
    if len(frame) < 4:
        raise SystemExit(2)
    header_length = int.from_bytes(frame[:4], "big")
    header_end = 4 + header_length
    header = json.loads(frame[4:header_end])
    source_offset = header_end
    owners = []
    for owner in header["owners"]:
        source_end = source_offset + owner["byteLength"]
        source = frame[source_offset:source_end]
        source_offset = source_end
        items = []
        if CONFIG["languageId"] == "rust":
            pattern = re.compile(rb"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
            for match in pattern.finditer(source):
                name = match.group(1).decode("utf-8")
                line_end = source.find(b"\n", match.start())
                if line_end < 0:
                    line_end = len(source)
                elif line_end == match.start():
                    line_end += 1
                selector = f"rust://{{owner['ownerPath']}}#item/function/{{name}}"
                items.append({{
                    "itemId": f"item:function:{{name}}",
                    "ownerId": f"owner:{{owner['ownerPath']}}",
                    "kind": "function",
                    "name": name,
                    "selector": selector,
                    "sourceByteStart": match.start(),
                    "sourceByteEnd": line_end,
                    "identity": {{
                        "schemaId": "asp.canonical-language-item-identity.v1",
                        "schemaVersion": "1",
                        "languageId": "rust",
                        "kind": "function",
                        "symbol": name,
                        "scopes": [],
                    }},
                    "projections": [],
                }})
        owners.append({{
            "ownerPath": owner["ownerPath"],
            "sourceLeafDigest": owner["sourceLeafDigest"],
            "items": items,
            "relations": [],
        }})
    if source_offset != len(frame):
        raise SystemExit(2)
    json.dump({{
        "schemaId": "asp.provider-language-projection-batch-response.v1",
        "schemaVersion": "1",
        "languageId": header["languageId"],
        "providerId": header["providerId"],
        "generationRootDigest": header["generationRootDigest"],
        "owners": owners,
    }}, sys.stdout, separators=(",", ":"))
    raise SystemExit(0)

if sys.argv[1] != "project-resolution-stdin":
    raise SystemExit(2)

request = json.load(sys.stdin)
extension = "." + CONFIG["extension"].lstrip(".")
source_roots = [root.strip("/") or "." for root in CONFIG["sourceRoots"]]
excluded_roots = [root.strip("/") for root in CONFIG["excludedRoots"]]
generation = request["candidateGeneration"]["digest"]

response = {{
    "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
    "schemaVersion": "1",
    "languageId": CONFIG["languageId"],
    "providerId": CONFIG["providerId"],
    "state": "resolved",
    "scope": {{
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": CONFIG["languageId"],
        "providerId": CONFIG["providerId"],
        "parserId": CONFIG["parserId"],
        "candidateGenerationDigest": generation,
        "projectEntry": CONFIG["projectEntry"],
        "packageGraph": {{
            "schemaId": "agent.semantic-protocols.language-package-graph",
            "schemaVersion": "1",
            "languageId": CONFIG["languageId"],
            "providerId": CONFIG["providerId"],
            "projectEntry": CONFIG["projectEntry"],
            "parserId": CONFIG["parserId"],
            "manifests": [{{"path": CONFIG["projectEntry"], "kind": "fixture-manifest", "digest": "fixture-manifest"}}],
            "lockfiles": [],
            "packages": [{{
                "packageId": "fixture",
                "name": "fixture",
                "manifestPath": CONFIG["projectEntry"],
                "root": ".",
                "workspaceMember": True,
                "targets": [{{
                    "targetId": "fixture:source",
                    "kind": "source",
                    "name": "fixture",
                    "explicit": True,
                    "sourceRoots": source_roots,
                    "entrypoints": [],
                    "generatedRoots": []
                }}]
            }}],
            "internalDependencyEdges": [],
            "externalDependencies": [],
            "unresolved": []
        }},
        "sourceScopes": [{{
            "scopeId": "fixture:source",
            "packageId": "fixture",
            "targetId": "fixture:source",
            "roots": source_roots,
            "explicitPaths": [],
            "extensions": [extension],
            "includeAuthority": "package-manager",
            "exclusions": [
                {{"prefix": root, "authority": "package-manager"}}
                for root in CONFIG["excludedRoots"]
            ],
        }}],
        "conflicts": [],
        "metrics": {{
            "parsedManifestCount": 1,
            "parsedLockfileCount": 0,
            "affectedPackageCount": 1,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 1
        }}
    }},
}}
json.dump(response, sys.stdout, separators=(",", ":"))
"#
    );
    std::fs::write(provider_bin, script).expect("write typed project-resolution provider");
    make_executable(provider_bin);
}

pub(crate) fn isolate_home(root: &Path) -> EnvVarGuard {
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("create isolated home");
    EnvVarGuard::set("HOME", home.as_os_str())
}

pub(super) fn home_local_provider_path(root: &Path, binary: &str) -> std::path::PathBuf {
    root.join("home")
        .join(".agent-semantic-protocols/runtime/bin")
        .join(binary)
}

pub(crate) struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    pub(super) fn set(name: &'static str, value: &OsStr) -> Self {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(self.name, value);
            } else {
                std::env::remove_var(self.name);
            }
        }
    }
}

pub(super) fn run_git(project_root: &Path, args: impl IntoIterator<Item = &'static str>) {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .expect("run git for source-index fixture");
    assert!(
        output.status.success(),
        "git source-index fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) struct RuntimeServerFixture {
    shutdown: agent_semantic_client_db::runtime_server::RuntimeServerShutdownHandle,
    task: tokio::task::JoinHandle<
        Result<agent_semantic_client_db::runtime_server::RuntimeServerExit, String>,
    >,
    runtime_base: std::path::PathBuf,
}

impl RuntimeServerFixture {
    pub(crate) async fn start(root: &Path) -> Self {
        let runtime_base = std::path::PathBuf::from("/tmp").join(format!(
            "asp-test-rs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("runtime fixture time")
                .as_nanos()
        ));
        let state_home = std::env::var_os("ASP_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join("home/.agent-semantic-protocols"));
        let artifact_catalog =
            agent_semantic_runtime::runtime_artifact_catalog::load_runtime_artifact_catalog(
                &state_home,
            )
            .await
            .expect("load fixture runtime artifact catalog");
        let endpoint =
            agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_in(
                &runtime_base,
                &std::env::current_exe().expect("current test executable"),
                "test-runtime-artifact-digest",
                artifact_catalog.mode_label(),
                &artifact_catalog.digest(),
                1,
                "test-source-index-binding",
            )
            .await
            .expect("prepare fixture Runtime Server endpoint");
        let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(&state_home);
        let server = agent_semantic_client_db::runtime_server::RuntimeServer::bind_and_publish_with_artifact_catalog(
            endpoint,
            std::sync::Arc::new(agent_semantic_client_db::WorkspaceDbRegistry::default()),
            &endpoint_path,
            std::sync::Arc::new(artifact_catalog),
        )
        .await
        .expect("bind fixture Runtime Server")
        .with_workspace_generation_builder(std::sync::Arc::new(
            |_workspace_identity, project_root| {
                Box::pin(async move {
                    crate::source_index::prepare_runtime_server_workspace_generation_async(
                        project_root,
                    )
                    .await
                })
            },
        ));
        let shutdown = server.shutdown_handle();
        let task = tokio::spawn(server.serve());
        Self {
            shutdown,
            task,
            runtime_base,
        }
    }

    pub(crate) async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        tokio::task::spawn_blocking(operation)
            .await
            .expect("join Runtime Server fixture client")
    }

    pub(crate) async fn rebuild(&self, root: &Path) {
        let session =
            agent_semantic_client_db::workspace_db_ipc::connect_runtime_server_workspace_session(
                root,
            )
            .await
            .expect("connect Runtime Server generation session");
        session
            .admit_runtime_generation()
            .await
            .expect("admit Runtime Server generation");
        let receipt = session
            .ensure_runtime_generation()
            .await
            .expect("ensure Runtime Server generation");
        assert_eq!(
            receipt.state,
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready,
            "fixture Runtime Server generation was not ready: {receipt:?}"
        );
    }

    pub(crate) async fn shutdown(self) {
        self.shutdown.shutdown();
        assert_eq!(
            self.task
                .await
                .expect("join fixture Runtime Server")
                .expect("serve fixture Runtime Server"),
            agent_semantic_client_db::runtime_server::RuntimeServerExit::ShutdownRequested
        );
        let _ = std::fs::remove_dir_all(self.runtime_base);
    }
}

pub(super) fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-client-source-index-{label}-{nanos}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    run_git(&root, ["init", "-q"]);
    root
}
