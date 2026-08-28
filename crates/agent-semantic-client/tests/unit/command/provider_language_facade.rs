use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn structural_item_source_query_does_not_use_cli_breaker() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://crates/agent-semantic-client/src/command/provider_dispatch.rs#item/function/run_language_command",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run public exact-query facade");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("provider command must be admitted as a Runtime Server route"),
        "public query must cross Runtime admission instead of the removed CLI breaker: {stderr}"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "public query must stop at the exact isolated Runtime authority terminal: {stdout}"
    );
}

#[test]
fn public_search_facades_use_runtime_admission_for_registered_languages() {
    for language_id in ["rust", "python", "gerbil-scheme"] {
        let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                language_id,
                "search",
                "pipe",
                "RuntimeAspClient",
                "--workspace",
                ".",
            ])
            .current_dir(workspace_root())
            .env("ASP_STATE_HOME", state_home.path())
            .output()
            .unwrap_or_else(|error| panic!("run public {language_id} search facade: {error}"));
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("provider command must be admitted as a Runtime Server route"),
            "public {language_id} search must cross Runtime admission: {stderr}"
        );
        assert!(
            stderr.is_empty(),
            "unexpected {language_id} stderr: {stderr}"
        );
        assert!(
            stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
            "public {language_id} search must stop at the exact isolated Runtime authority terminal: {stdout}"
        );
    }
}

#[test]
fn structural_item_source_query_requires_current_runtime_authority_before_provider_resolution() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
            "query",
            "--selector",
            "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run asp structural item query");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr={stderr}");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "stdout={stdout}"
    );
}

#[test]
fn exact_structural_selector_does_not_require_a_term() {
    let state_home = tempfile::tempdir().expect("isolated Runtime State Home");
    let selector = "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery";
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "typescript",
            "query",
            "--selector",
            selector,
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .current_dir(workspace_root())
        .env("ASP_STATE_HOME", state_home.path())
        .output()
        .expect("run exact structural selector query");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr={stderr}");
    assert!(
        !stderr.contains("query requires at least one --term"),
        "stderr={stderr}"
    );
    assert!(
        stdout.contains("\"reasonKind\":\"runtime-server-activation-unavailable\""),
        "stdout={stdout}"
    );
}

#[test]
fn removed_exact_query_flags_are_rejected_by_cli_admission() {
    for removed_flag in ["--code", "--names-only"] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([
                "typescript",
                "query",
                "--selector",
                "typescript://languages/typescript-lang-project-harness/src/cli/semantic-search/item-query.ts#item/function/renderOwnerItemQuery",
                "--workspace",
                ".",
                removed_flag,
            ])
            .current_dir(workspace_root())
            .output()
            .expect("run removed exact-query flag");

        assert!(!output.status.success(), "{removed_flag} was admitted");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unexpected argument"),
            "removed flag did not use ordinary CLI rejection: flag={removed_flag} stderr={stderr}"
        );
        assert!(
            !stderr.contains("legacy") && !stderr.contains("unsupported"),
            "removed flag leaked compatibility guidance: flag={removed_flag} stderr={stderr}"
        );
    }
}

#[test]
fn document_exact_selector_crosses_the_language_neutral_owner_boundary() {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "org",
            "query",
            "--selector",
            "org://docs/missing.org#item/heading/missing",
            "--workspace",
            ".",
            "--projection",
            "content",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run org exact structural selector query");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("provider-owned structural query is missing an exact owner path"),
        "org selector was rejected by a language-hardcoded owner boundary: {stderr}"
    );
    assert!(
        !stderr.contains("query requires at least one --term"),
        "org exact selector was misrouted into lexical query: {stderr}"
    );
}

#[tokio::test]
async fn production_language_query_spawn_acceptance_terminalizes_before_endpoint_read() {
    let _install_guard = crate::install_binary_test_guard::acquire();
    let temporary = tempfile::tempdir().expect("production command fixture root");
    let state_resolution = agent_semantic_runtime::state_core::resolve_state_home_projection_from(
        None,
        Some(temporary.path().as_os_str().to_os_string()),
    )
    .expect("resolve isolated HOME through the production State Home owner");
    assert_eq!(
        state_resolution.source,
        agent_semantic_runtime::state_core::StateHomeResolutionSource::HomeDefault
    );
    assert!(!state_resolution.asp_state_home_present);
    assert!(state_resolution.home_present);
    let state_home = state_resolution.state_home;
    assert_eq!(
        state_home,
        temporary
            .path()
            .canonicalize()
            .expect("canonical fixture HOME")
            .join(".agent-semantic-protocols")
    );
    let project_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&project_root).expect("create fixture workspace");
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repository root");
    let production_binary = repository_root
        .join("target/debug/asp")
        .canonicalize()
        .expect("canonical target/debug/asp");
    assert_eq!(
        std::path::Path::new(env!("CARGO_BIN_EXE_asp"))
            .canonicalize()
            .expect("canonical Cargo-built asp"),
        production_binary,
        "the production fixture must execute the real target/debug/asp"
    );
    std::fs::create_dir_all(&state_home).expect("create resolved State Home");
    std::fs::write(
        state_home.join("asp.toml"),
        format!("[dev]\nenabled = true\nroot = {:?}\n", repository_root),
    )
    .expect("configure the production Developer source root");
    let install = std::process::Command::new(&production_binary)
        .env_remove("ASP_STATE_HOME")
        .env("HOME", temporary.path())
        .current_dir(&repository_root)
        .args(["install", "binary"])
        .output()
        .expect("run production install binary command");
    assert!(
        install.status.success(),
        "install binary must publish a pending Runtime activation: stdout={} stderr={}",
        String::from_utf8_lossy(&install.stdout),
        String::from_utf8_lossy(&install.stderr)
    );
    let install_stdout = String::from_utf8_lossy(&install.stdout);
    assert!(
        install_stdout.contains(&format!("stateHome={}", state_home.display()))
            && install_stdout.contains("stateHomeSource=HomeDefault")
            && install_stdout.contains("aspStateHomePresent=false")
            && install_stdout.contains("installScope=global")
            && install_stdout.contains(&format!(
                "pendingActivationPath={}",
                agent_semantic_artifacts::runtime_artifact_publication::runtime_artifact_activation_event_path(&state_home).display()
            )),
        "install did not project the canonical State Home authority: {install_stdout}"
    );
    let activation =
        agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(
            &state_home,
        )
        .await
        .expect("read production activation event")
        .expect("install binary must leave a consumable activation generation");
    assert!(activation.activation_generation > 0);
    assert!(activation.artifact_path.is_file());
    let applied =
        agent_semantic_artifacts::runtime_artifact_publication::commit_runtime_artifact_activation(
            &state_home,
            &activation,
            None,
        )
        .await
        .expect("commit the first production activation generation");
    assert_eq!(
        applied.activation_generation,
        activation.activation_generation
    );
    assert_eq!(applied.artifact_digest, activation.artifact_digest);
    assert!(
        agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(
            &state_home,
        )
        .await
        .expect("read applied first production activation")
        .is_none()
    );

    let reinstall = std::process::Command::new(&production_binary)
        .env_remove("ASP_STATE_HOME")
        .env("HOME", temporary.path())
        .current_dir(&repository_root)
        .args(["install", "binary"])
        .output()
        .expect("repeat production install binary command");
    assert!(
        reinstall.status.success(),
        "repeated install must publish a newer pending generation: stdout={} stderr={}",
        String::from_utf8_lossy(&reinstall.stdout),
        String::from_utf8_lossy(&reinstall.stderr)
    );
    let republished =
        agent_semantic_artifacts::runtime_artifact_publication::read_runtime_artifact_activation_event(
            &state_home,
        )
        .await
        .expect("read repeated production activation")
        .expect("stale applied generation must not hide the repeated install");
    assert_eq!(republished.artifact_digest, activation.artifact_digest);
    assert!(republished.activation_generation > activation.activation_generation);
    let legacy_owner_path = state_home.join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(
        legacy_owner_path
            .parent()
            .expect("legacy owner receipt parent"),
    )
    .expect("create legacy owner receipt parent");
    std::fs::write(
        &legacy_owner_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
            "processId": u32::MAX,
            "nonce": "stale-owner-before-new-pending",
            "stateHome": state_home,
            "runtimeArtifactPath": activation.artifact_path,
        }))
        .expect("encode legacy owner receipt"),
    )
    .expect("publish legacy owner observation");

    let output = std::process::Command::new(&production_binary)
        .env_remove("ASP_STATE_HOME")
        .env("HOME", temporary.path())
        .current_dir(&project_root)
        .args([
            "rust",
            "query",
            "--selector",
            "rust://crates/agent-semantic-client/src/command/provider_dispatch.rs#item/function/run_language_command",
            "--projection",
            "callable-skeleton",
            "--workspace",
            project_root.to_str().expect("UTF-8 fixture workspace"),
        ])
        .output()
        .expect("run production language query command");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 command stdout");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 command stderr");
    assert!(output.status.success(), "command failed: {stderr}");
    assert!(
        stdout.contains("agent.semantic-protocols.runtime-server-client-bootstrap-receipt"),
        "missing typed bootstrap receipt: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("runtime-server-activation-spawn-accepted"),
        "missing SpawnAccepted terminal: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("\"stateHomeSource\":\"home-default\"")
            && stdout.contains("\"aspStateHomePresent\":false")
            && stdout.contains(&state_home.display().to_string()),
        "bootstrap did not project the same State Home authority: {stdout}"
    );
    assert!(
        !stdout.contains("runtime-server-endpoint-unavailable")
            && !stderr.contains("runtime-server-endpoint-unavailable"),
        "production command read the endpoint after SpawnAccepted: stdout={stdout} stderr={stderr}"
    );
    let claimed_owner =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read claimed owner receipt")
            .expect("pending activation must publish a current owner receipt");
    assert_eq!(
        claimed_owner.activation_generation,
        republished.activation_generation
    );
    assert_eq!(
        claimed_owner.launcher_artifact_digest,
        republished.artifact_digest
    );
}

#[test]
fn production_language_query_without_activation_fails_at_bootstrap_authority() {
    let temporary = tempfile::tempdir().expect("missing activation fixture root");
    let state_resolution = agent_semantic_runtime::state_core::resolve_state_home_projection_from(
        None,
        Some(temporary.path().as_os_str().to_os_string()),
    )
    .expect("resolve isolated HOME through the production State Home owner");
    assert_eq!(
        state_resolution.source,
        agent_semantic_runtime::state_core::StateHomeResolutionSource::HomeDefault
    );
    assert!(!state_resolution.asp_state_home_present);
    assert!(state_resolution.home_present);
    let state_home = state_resolution.state_home;
    assert_eq!(
        state_home,
        temporary
            .path()
            .canonicalize()
            .expect("canonical fixture HOME")
            .join(".agent-semantic-protocols")
    );
    let project_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&project_root).expect("create fixture workspace");
    std::fs::create_dir_all(&state_home).expect("create resolved State Home without activation");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .env_remove("ASP_STATE_HOME")
        .env("HOME", temporary.path())
        .current_dir(&project_root)
        .args([
            "rust",
            "query",
            "--selector",
            "rust://crates/agent-semantic-client/src/command/provider_dispatch.rs#item/function/run_language_command",
            "--projection",
            "callable-skeleton",
            "--workspace",
        ])
        .arg(&project_root)
        .output()
        .expect("run production command without activation authority");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 command stdout");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 command stderr");
    assert!(output.status.success(), "command failed: {stderr}");
    assert!(
        stdout.contains("agent.semantic-protocols.runtime-server-client-bootstrap-receipt"),
        "missing typed bootstrap receipt: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("runtime-server-activation-unavailable"),
        "missing activation-authority terminal: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("\"stateHomeSource\":\"home-default\"")
            && stdout.contains("\"aspStateHomePresent\":false")
            && stdout.contains(&state_home.display().to_string()),
        "missing-activation terminal did not project State Home authority: {stdout}"
    );
    assert!(
        !stdout.contains("runtime-server-endpoint-unavailable")
            && !stderr.contains("runtime-server-endpoint-unavailable"),
        "missing activation leaked into endpoint handling: stdout={stdout} stderr={stderr}"
    );
}

#[test]
fn production_language_query_without_pending_rejects_stale_owner_authority() {
    let temporary = tempfile::tempdir().expect("stale owner fixture root");
    let state_resolution = agent_semantic_runtime::state_core::resolve_state_home_projection_from(
        None,
        Some(temporary.path().as_os_str().to_os_string()),
    )
    .expect("resolve isolated HOME through the production State Home owner");
    let state_home = state_resolution.state_home;
    let project_root = temporary.path().join("workspace");
    std::fs::create_dir_all(&project_root).expect("create fixture workspace");
    let owner_path = state_home.join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(owner_path.parent().expect("owner receipt parent"))
        .expect("create owner receipt parent");
    std::fs::write(
        owner_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
            "processId": u32::MAX,
            "nonce": "stale-owner-without-pending",
            "stateHome": state_home,
            "runtimeArtifactPath": state_home.join("runtime/bin/asp"),
        }))
        .expect("encode legacy owner receipt"),
    )
    .expect("publish legacy owner observation");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_asp"))
        .env_remove("ASP_STATE_HOME")
        .env("HOME", temporary.path())
        .current_dir(&project_root)
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/lib.rs#item/function/missing",
            "--projection",
            "source",
            "--workspace",
        ])
        .arg(&project_root)
        .output()
        .expect("run production command with stale owner authority");
    let output_text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success());
    assert!(output_text.contains("runtime-authority-stale"));
    assert!(!output_text.contains("runtime-server-activation-unavailable"));
}
