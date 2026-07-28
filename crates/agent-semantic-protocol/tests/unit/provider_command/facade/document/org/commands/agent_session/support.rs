use crate::provider_command::support::{make_executable, write_marker_provider};
use agent_semantic_hook::builtin_provider_manifests;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) struct CodexAspExplorerFixtureProfile<'a> {
    expected_model: &'a str,
    actual_model: &'a str,
    expected_sandbox: &'a str,
    actual_sandbox: &'a str,
}

impl<'a> CodexAspExplorerFixtureProfile<'a> {
    pub(super) fn new(
        expected_model: &'a str,
        actual_model: &'a str,
        expected_sandbox: &'a str,
        actual_sandbox: &'a str,
    ) -> Self {
        Self {
            expected_model,
            actual_model,
            expected_sandbox,
            actual_sandbox,
        }
    }
}

struct CodexAspExplorerFixtureAgent<'a> {
    actual_agent_path: Option<&'a Path>,
    include_agent_path: bool,
    actual_agent_role: &'a str,
}

pub(super) fn install_rust_marker_provider(state_home: &Path) {
    install_rust_provider(state_home, None);
}

pub(super) fn install_rust_owner_frontier_provider(state_home: &Path) {
    install_rust_provider(
        state_home,
        Some(
            "#!/bin/sh\nprintf '[search-owner] q=message_target_snapshot pkg=. selector=items alg=item-frontier\\n'\nprintf 'O=owner:path(crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs)!owner;I=item:symbol(message_target_snapshot)!syntax\\n'\nprintf 'O>{I:contains}\\n'\nprintf 'rank=I,O frontier=I.syntax\\n'\n",
        ),
    );
}

fn install_rust_provider(state_home: &Path, delegate: Option<&str>) {
    let manifest = builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("builtin Rust provider manifest");
    let runtime_bin = state_home.join("runtime/bin");
    write_marker_provider(
        &runtime_bin,
        manifest.binary(),
        &runtime_bin.join(".rs-harness-marker"),
    );
    if let Some(delegate) = delegate {
        let delegate_path = runtime_bin.join(".rs-harness-delegate");
        std::fs::write(&delegate_path, delegate).expect("write Rust provider delegate");
        make_executable(&delegate_path);
    }
    let installed = runtime_bin.join(manifest.binary());
    let content_digest = agent_semantic_content_identity::file_content_digest_v1(&installed)
        .expect("installed provider content digest");
    let metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&installed)
            .expect("installed provider metadata digest");
    let lock_dir = agent_semantic_runtime::provider_receipt_dir(&state_home);
    std::fs::create_dir_all(&lock_dir).expect("create provider lock registry");
    std::fs::write(
        lock_dir.join("rust.lock.toml"),
        format!(
            "schemaId = \"asp.provider-install-lock.v1\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\n",
            manifest.provider_id(),
            installed.display(),
            content_digest,
            metadata_digest,
        ),
    )
    .expect("write Rust provider install receipt");
}

pub(super) fn write_codex_asp_explorer_fixture(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    model: &str,
    sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_sandbox(
        home,
        root_session_id,
        child_session_id,
        model,
        sandbox,
        sandbox,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_sandbox(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_profile(
        home,
        root_session_id,
        child_session_id,
        model,
        model,
        expected_sandbox,
        actual_sandbox,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_profile(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_actual_agent_path(
        home,
        root_session_id,
        child_session_id,
        CodexAspExplorerFixtureProfile::new(
            expected_model,
            actual_model,
            expected_sandbox,
            actual_sandbox,
        ),
        None,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_actual_agent_path(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    profile: CodexAspExplorerFixtureProfile<'_>,
    actual_agent_path: Option<&Path>,
) {
    write_codex_asp_explorer_fixture_with_agent_path_presence(
        home,
        root_session_id,
        child_session_id,
        profile,
        actual_agent_path,
        true,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_without_agent_path(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_agent_path_presence(
        home,
        root_session_id,
        child_session_id,
        CodexAspExplorerFixtureProfile::new(
            expected_model,
            actual_model,
            expected_sandbox,
            actual_sandbox,
        ),
        None,
        false,
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_agent_path_presence(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    profile: CodexAspExplorerFixtureProfile<'_>,
    actual_agent_path: Option<&Path>,
    include_agent_path: bool,
) {
    write_codex_asp_explorer_fixture_with_agent_role(
        home,
        root_session_id,
        child_session_id,
        profile,
        CodexAspExplorerFixtureAgent {
            actual_agent_path,
            include_agent_path,
            actual_agent_role: "asp_explorer",
        },
    );
}

pub(super) fn write_codex_asp_explorer_fixture_with_default_agent_role(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    expected_model: &str,
    actual_model: &str,
    expected_sandbox: &str,
    actual_sandbox: &str,
) {
    write_codex_asp_explorer_fixture_with_agent_role(
        home,
        root_session_id,
        child_session_id,
        CodexAspExplorerFixtureProfile::new(
            expected_model,
            actual_model,
            expected_sandbox,
            actual_sandbox,
        ),
        CodexAspExplorerFixtureAgent {
            actual_agent_path: None,
            include_agent_path: false,
            actual_agent_role: "default",
        },
    );
}

fn write_codex_asp_explorer_fixture_with_agent_role(
    home: &Path,
    root_session_id: &str,
    child_session_id: &str,
    profile: CodexAspExplorerFixtureProfile<'_>,
    agent: CodexAspExplorerFixtureAgent<'_>,
) {
    let CodexAspExplorerFixtureProfile {
        expected_model,
        actual_model,
        expected_sandbox,
        actual_sandbox,
    } = profile;
    let CodexAspExplorerFixtureAgent {
        actual_agent_path,
        include_agent_path,
        actual_agent_role,
    } = agent;
    let agents_dir = home.join(".codex").join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create codex agents dir");
    let expected_agent_path = agents_dir.join("asp-explorer.toml");
    std::fs::write(
        &expected_agent_path,
        format!(
            "name = \"asp_explorer\"\nmodel = \"{expected_model}\"\nsandbox_mode = \"{expected_sandbox}\"\nsession_lifetime = \"resident\"\n"
        ),
    )
    .expect("write asp explorer config");
    let rollout_agent_path = actual_agent_path
        .unwrap_or(expected_agent_path.as_path())
        .display()
        .to_string();

    let rollout_dir = home
        .join(".codex")
        .join("sessions")
        .join("2026")
        .join("07")
        .join("01");
    std::fs::create_dir_all(&rollout_dir).expect("create codex rollout dir");
    let root_rollout_path = rollout_dir.join(format!("rollout-test-{root_session_id}.jsonl"));
    let root_session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": root_session_id
        }
    });
    let child_spawn_output =
        serde_json::json!({"agent_id": child_session_id, "nickname": "ASP search"});
    let child_spawn = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "output": child_spawn_output.to_string()
        }
    });
    std::fs::write(
        root_rollout_path,
        format!("{root_session_meta}\n{child_spawn}\n"),
    )
    .expect("write root codex rollout");
    let rollout_path = rollout_dir.join(format!("rollout-test-{child_session_id}.jsonl"));
    let mut session_meta = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": child_session_id,
            "parent_thread_id": root_session_id,
            "thread_source": "subagent",
            "agent_role": actual_agent_role,
            "agent_nickname": "ASP search",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_role": actual_agent_role,
                        "agent_nickname": "ASP search"
                    }
                }
            }
        }
    });
    if include_agent_path {
        session_meta["payload"]["source"]["subagent"]["thread_spawn"]["agent_path"] =
            serde_json::json!(rollout_agent_path);
    }
    let turn_context = serde_json::json!({
        "type": "turn_context",
        "payload": {
            "model": actual_model,
            "sandbox_policy": {"type": actual_sandbox},
            "approval_policy": "never",
            "permission_profile": {"type": "disabled"}
        }
    });
    std::fs::write(rollout_path, format!("{session_meta}\n{turn_context}\n"))
        .expect("write codex rollout");
}
