use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use agent_semantic_client_db::{ClientDbEngine, ClientDbProviderCommandSelection};

#[test]
fn db_engine_provider_command_selections_use_active_turso_path_without_retired_db_control() {
    let client_dir = temp_root("db-engine-provider-selection-client");
    let project_root = temp_root("db-engine-provider-selection-project");
    let row = provider_command_selection_fixture("rust", "rs-harness", "sha256:abc");
    let context_b_row = provider_command_selection_fixture("python", "py-harness", "sha256:def");

    ClientDbEngine::replace_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        "sha256:ctx-a",
        std::slice::from_ref(&row),
    )
    .expect("write Turso provider command selections");
    ClientDbEngine::replace_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        "sha256:ctx-b",
        std::slice::from_ref(&context_b_row),
    )
    .expect("write second Turso provider command context");
    let hit = ClientDbEngine::lookup_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        "sha256:ctx-a",
    )
    .expect("lookup Turso provider command selections")
    .expect("provider selection rows");
    let context_b_hit = ClientDbEngine::lookup_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        "sha256:ctx-b",
    )
    .expect("lookup second Turso provider command context")
    .expect("second provider selection rows");
    let miss = ClientDbEngine::lookup_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        "sha256:missing",
    )
    .expect("lookup Turso provider command selection miss");

    assert_eq!(hit, vec![row]);
    assert_eq!(context_b_hit, vec![context_b_row]);
    assert!(miss.is_none());
    assert!(client_dir.join("client.turso").exists());
    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn db_engine_provider_command_process_writer_helper() {
    if env::var("ASP_TURSO_PROVIDER_PROCESS_STRESS_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let client_dir = PathBuf::from(
        env::var("ASP_TURSO_PROVIDER_PROCESS_STRESS_CLIENT_DIR")
            .expect("ASP_TURSO_PROVIDER_PROCESS_STRESS_CLIENT_DIR"),
    );
    let project_root = PathBuf::from(
        env::var("ASP_TURSO_PROVIDER_PROCESS_STRESS_PROJECT_ROOT")
            .expect("ASP_TURSO_PROVIDER_PROCESS_STRESS_PROJECT_ROOT"),
    );
    let writer_id: usize = env::var("ASP_TURSO_PROVIDER_PROCESS_STRESS_WRITER_ID")
        .expect("ASP_TURSO_PROVIDER_PROCESS_STRESS_WRITER_ID")
        .parse()
        .expect("parse ASP_TURSO_PROVIDER_PROCESS_STRESS_WRITER_ID");
    let selection = provider_command_selection_fixture(
        "rust",
        &format!("rs-harness-{writer_id}"),
        &format!("sha256:provider-{writer_id}"),
    );
    ClientDbEngine::replace_provider_command_selections_from_client_dir(
        &client_dir,
        &project_root,
        &format!("sha256:ctx-process-{writer_id}"),
        &[selection],
    )
    .expect("process writer should write Turso provider command selection");
}

#[test]
fn db_engine_provider_command_writes_survive_concurrent_agent_process_stress() {
    let client_dir = temp_root("db-engine-provider-selection-concurrent-client");
    let project_root = temp_root("db-engine-provider-selection-concurrent-project");
    let writer_count = 6usize;
    let current_exe = env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();

    for writer_id in 0..writer_count {
        children.push(
            Command::new(&current_exe)
                .arg("--exact")
                .arg("db_engine_provider_command::db_engine_provider_command_process_writer_helper")
                .arg("--nocapture")
                .env("ASP_TURSO_PROVIDER_PROCESS_STRESS_CHILD", "1")
                .env("ASP_TURSO_PROVIDER_PROCESS_STRESS_CLIENT_DIR", &client_dir)
                .env(
                    "ASP_TURSO_PROVIDER_PROCESS_STRESS_PROJECT_ROOT",
                    &project_root,
                )
                .env(
                    "ASP_TURSO_PROVIDER_PROCESS_STRESS_WRITER_ID",
                    writer_id.to_string(),
                )
                .spawn()
                .expect("spawn process provider-command writer"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait for process provider writer");
        assert!(status.success(), "process provider writer failed: {status}");
    }

    for writer_id in 0..writer_count {
        let hit = ClientDbEngine::lookup_provider_command_selections_from_client_dir(
            &client_dir,
            &project_root,
            &format!("sha256:ctx-process-{writer_id}"),
        )
        .expect("lookup process provider command context")
        .expect("process provider command context exists");
        assert_eq!(
            hit,
            vec![provider_command_selection_fixture(
                "rust",
                &format!("rs-harness-{writer_id}"),
                &format!("sha256:provider-{writer_id}"),
            )]
        );
    }

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

fn provider_command_selection_fixture(
    language_id: &str,
    provider_id: &str,
    manifest_digest: &str,
) -> ClientDbProviderCommandSelection {
    ClientDbProviderCommandSelection::new(
        format!("agent.semantic-protocols.languages.{language_id}.{provider_id}"),
        manifest_digest.to_string(),
        language_id.to_string(),
        provider_id.to_string(),
        provider_id.to_string(),
        "external-process".to_string(),
        vec![format!("/tmp/{provider_id}")],
        Some(format!("/tmp/{provider_id}")),
        Some(42),
        Some(1234),
    )
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    env::temp_dir().join(format!("asp-client-db-{name}-{nanos}"))
}
