// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Production-command coverage for the real Cargo-built ASP executable.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

struct IsolatedStateHome(PathBuf);

impl IsolatedStateHome {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "asp-install-binary-production-command-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create isolated ASP State Home");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for IsolatedStateHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn built_asp_install_binary_publishes_runtime_hook_independent_of_runtime_activation() {
    let _install_guard = crate::install_binary_test_guard::acquire();
    let state_home = IsolatedStateHome::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let built_asp = Path::new(env!("CARGO_BIN_EXE_asp"));
    assert!(
        built_asp.is_file(),
        "Cargo-built asp binary must exist: {}",
        built_asp.display()
    );
    let build_fixture = tempfile::tempdir().expect("isolated binary build directory");
    let asp = build_fixture.path().join("asp");
    let hook = build_fixture.path().join("asp-hook");
    std::fs::copy(built_asp, &asp).expect("copy ASP binary fixture");
    let embedded = agent_semantic_hook::aot_compiler::compile_embedded_hook_policy_bundle()
        .expect("compile embedded policy identity");
    let embedded: serde_json::Value =
        serde_json::from_slice(&embedded).expect("decode embedded policy identity");
    let policy_digest = embedded["generationDigest"]
        .as_str()
        .expect("embedded policy digest");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{{\"schemaId\":\"agent.semantic-protocols.hook-runtime-identity\",\"schemaVersion\":1,\"policyContentDigest\":\"{policy_digest}\",\"handlerElapsedNanos\":1}}'\n"
            ),
        )
        .expect("write Hook identity fixture");
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook, permissions).expect("Hook identity fixture mode");
    }
    let codex_home = state_home.path().join("codex-home");
    let plugin_cache = codex_home.join("plugins/cache/asp-project/asp-codex-plugin/current");
    std::fs::create_dir_all(&plugin_cache).expect("create isolated plugin cache");
    std::fs::write(
        plugin_cache.join("payload.marker"),
        b"plugin-cache-must-not-change",
    )
    .expect("write isolated plugin cache marker");
    let plugin_cache_before = directory_identity(&plugin_cache);
    let plugin_source = workspace_root.join("asp-codex-plugin");
    let plugin_source_before = directory_identity(&plugin_source);
    let isolated_home = state_home.path().join("home");
    let output = Command::new(&asp)
        .args(["install", "binary"])
        .current_dir(workspace_root)
        .env("ASP_STATE_HOME", state_home.path())
        .env("HOME", &isolated_home)
        .env("CODEX_HOME", &codex_home)
        .output()
        .expect("execute actual Cargo-built asp install binary");
    assert!(
        output.status.success(),
        "actual asp install binary must publish an immutable activation candidate: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let install_stdout = String::from_utf8(output.stdout).expect("install receipt UTF-8");
    assert!(install_stdout.contains("hookBinaryPath="));
    assert!(install_stdout.contains("hookBinaryDigest=blake3-256:"));
    assert!(install_stdout.contains("hookBinarySwitch=active-healthy-bundle"));
    assert!(install_stdout.contains("runtimeServerLifecycle=resident-owner-independent"));
    assert!(install_stdout.contains("hookAuthority=runtime-active-bundle-content-digest"));
    assert_eq!(
        directory_identity(&plugin_source),
        plugin_source_before,
        "binary installation must not rewrite the plugin source payload"
    );
    assert_eq!(
        directory_identity(&plugin_cache),
        plugin_cache_before,
        "binary installation must not mutate the global Codex plugin cache"
    );

    let installed_hook = state_home.path().join("runtime/artifacts/active/asp-hook");
    assert!(
        installed_hook.exists(),
        "canonical Hook evaluator publication"
    );
    assert!(!state_home.path().join("hooks/current").exists());

    let pending_path = agent_semantic_artifacts::runtime_artifact_activation::
        runtime_artifact_activation_event_path(state_home.path());
    let activation = serde_json::from_slice::<
        agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent,
    >(&std::fs::read(&pending_path).expect("read pending activation receipt"))
    .expect("decode pending activation receipt");
    let expected_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&asp).expect("read Cargo-built asp binary"),
        );
    assert!(!activation.publication_nonce.is_empty());
    assert_eq!(activation.artifact_digest, expected_digest);
    assert!(activation.artifact_path.is_file());
    assert!(activation.artifact_path.components().any(|component| {
        component.as_os_str() == activation.bundle_digest.content_digest().as_str()
    }));
    assert!(
        !state_home
            .path()
            .join("runtime/artifacts/activation/applied.json")
            .exists(),
        "install cannot claim applied authority before the Runtime actor commits"
    );
    assert!(
        !state_home
            .path()
            .join("runtime/server/endpoint.v1.json")
            .exists(),
        "install cannot publish a Runtime endpoint"
    );
    assert_eq!(
        std::fs::canonicalize(state_home.path().join("runtime/bin/asp"))
            .expect("canonical client launcher"),
        std::fs::canonicalize(&activation.artifact_path)
            .expect("canonical pending activation artifact"),
        "install must switch only the client launcher to the pending immutable candidate"
    );
    let path_visible_asp = isolated_home.join(".local/bin/asp");
    assert_eq!(
        std::fs::read_link(state_home.path().join("runtime/bin/asp"))
            .expect("read Runtime compatibility alias"),
        path_visible_asp,
        "Runtime compatibility alias must point to the PATH-visible install"
    );
    assert_ne!(
        std::fs::read_link(&path_visible_asp).expect("read PATH-visible ASP entry"),
        state_home.path().join("runtime/bin/asp"),
        "PATH-visible ASP entry must not point back to the Runtime alias"
    );
    assert!(
        state_home.path().join("runtime/artifacts/active").exists(),
        "install must switch the sole active generation selector"
    );
    assert!(
        !state_home
            .path()
            .join("runtime/artifacts/leases/artifact-publication.v1.json")
            .exists(),
        "successful production command consumes its publication lease exactly once"
    );

    let pending_bytes = std::fs::read(&pending_path).expect("preserve pending Runtime state");
    let reader_fixture = agent_semantic_hook::materialize_reader_probe_fixture()
        .expect("materialize canonical Reader probe fixture");
    let reader_subject = "crates/agent-semantic-hook/src/lib.rs";
    let mut launcher_elapsed_micros = Vec::new();
    for runtime_state in ["pending", "stopped", "failed"] {
        match runtime_state {
            "pending" => std::fs::write(&pending_path, &pending_bytes).expect("restore pending"),
            "stopped" => {
                let _ = std::fs::remove_file(&pending_path);
                let _ = std::fs::remove_file(
                    state_home
                        .path()
                        .join("runtime/artifacts/activation/failed.json"),
                );
            }
            "failed" => {
                let _ = std::fs::remove_file(&pending_path);
                std::fs::write(
                    state_home
                        .path()
                        .join("runtime/artifacts/activation/failed.json"),
                    br#"{"state":"failed"}"#,
                )
                .expect("write failed Runtime observation");
            }
            _ => unreachable!(),
        }
        let mut spec = agent_semantic_hook_testkit::HookProcessSpec::new(
            workspace_root.join("asp-codex-plugin/bin/asp-hook-exec"),
            workspace_root,
        );
        spec.args = vec![
            "pre-tool".to_owned(),
            "--client".to_owned(),
            "codex".to_owned(),
            "--host-match".to_owned(),
            "Bash".to_owned(),
        ];
        spec.env.push((
            "ASP_STATE_HOME".to_owned(),
            state_home.path().display().to_string(),
        ));
        spec.env
            .push(("ASP_HOOK_BOOTSTRAP_TRACE".to_owned(), "1".to_owned()));
        spec.timeout = std::time::Duration::from_secs(10);
        let receipt = agent_semantic_hook_testkit::run_hook_process(
            &spec,
            &serde_json::json!({
                "session_id": format!("runtime-{runtime_state}"),
                "cwd": workspace_root,
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {
                    "command": format!("{} read {reader_subject}", reader_fixture.display())
                }
            }),
        )
        .await
        .unwrap_or_else(|error| panic!("Hook launcher under Runtime {runtime_state}: {error}"));
        assert_eq!(
            receipt.decision["hookSpecificOutput"]["permissionDecision"], "deny",
            "Runtime {runtime_state}: {}",
            receipt.decision
        );
        let context = receipt.decision["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("generation-bound deny context");
        assert!(
            context.contains(policy_digest),
            "Runtime {runtime_state}: {context}"
        );
        assert!(
            context.contains("\"access\":\"read\"")
                && context.contains("\"accessMode\":\"read-permission\"")
                && (context.contains("\"evidence\":\"reader-behavior-dynamic-cache\"")
                    || context.contains("\"evidence\":\"reader-probe-read-permission\"")),
            "Runtime {runtime_state}: {context}"
        );
        assert!(
            !context.contains("runtime-server"),
            "Runtime {runtime_state}: {context}"
        );
        assert!(
            receipt.elapsed < std::time::Duration::from_secs(1),
            "Runtime {runtime_state}: Hook launcher exceeded Host deadline: elapsed={:?} stderr={}",
            receipt.elapsed,
            receipt.stderr
        );
        launcher_elapsed_micros.push((runtime_state, receipt.elapsed.as_micros()));
    }
    eprintln!(
        "hook-runtime-production-receipt policy={} runtimeStates={launcher_elapsed_micros:?}",
        policy_digest
    );
}

fn directory_identity(root: &Path) -> String {
    let mut files = Vec::new();
    collect_files(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = blake3::Hasher::new();
    for (relative, bytes) in files {
        hasher.update(relative.as_bytes());
        hasher.update(&[0]);
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let mut entries = std::fs::read_dir(current)
        .unwrap_or_else(|error| panic!("read {}: {error}", current.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("collect {}: {error}", current.display()));
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push((
                path.strip_prefix(root)
                    .expect("file under identity root")
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
            ));
        }
    }
}
