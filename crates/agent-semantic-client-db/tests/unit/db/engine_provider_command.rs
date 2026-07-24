use std::{
    env, fs,
    path::PathBuf,
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
    assert!(client_dir.join("facts.turso").exists());
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
