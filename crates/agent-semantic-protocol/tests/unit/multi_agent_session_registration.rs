use super::{
    child_registration_receipt_schema_is_current, read_or_materialize_registration_handoff,
};
use crate::multi_agent_session::{
    ChildSessionRegistrationReceipt, child_registration_control_plane_projection,
    child_registration_path, publish_child_registration_receipt_at_path,
    read_child_registration_at_path, read_replaceable_child_registration_at_path,
};
use crate::multi_agent_session::{
    read_current_child_registration_at_path, read_current_child_registrations_for_route_at_root,
};

fn current_registration_receipt_fixture() -> ChildSessionRegistrationReceipt {
    serde_json::from_value(serde_json::json!({
        "schemaId": "agent.child-session-registration",
        "schemaVersion": 1,
        "projectId": "project",
        "rootSessionId": "root",
        "parentSessionId": "parent",
        "childSessionId": "child",
        "hostRole": "explorer",
        "residentId": "asp_explorer",
        "canonicalAgentName": "asp_explorer",
        "platform": "codex",
        "routeKey": "asp_explorer",
        "routeDigest": format!("blake3-256:{}", "a".repeat(64)),
        "profileId": "agents/asp_explorer_codex.toml",
        "profileDigest": format!("blake3-256:{}", "b".repeat(64)),
        "policyDigest": format!("blake3-256:{}", "c".repeat(64)),
        "definitionSchemaId": "urn:agent-semantic-protocols:schema:codex-agent-definition",
        "deniedActions": ["edit"],
        "allowedRuleIntents": ["reasoning-search", "structured-projection"],
        "sandboxMode": "read-only",
        "sessionLifetime": "resident",
        "generation": 1,
        "lifecycleState": "live",
        "routable": true,
        "hostCallReceiptDigest": format!("blake3-256:{}", "d".repeat(64)),
        "registrationAuthority": "project-child-registration-authority"
    }))
    .expect("decode current registration receipt fixture")
}

#[test]
fn stable_v1_receipt_schema_requires_generation_and_all_route_policy_bindings() {
    let current = current_registration_receipt_fixture();
    assert!(child_registration_receipt_schema_is_current(&current));

    let mut incomplete = serde_json::to_value(&current).expect("encode receipt fixture");
    incomplete["schemaId"] = serde_json::json!("agent.child-session-registration");
    incomplete["schemaVersion"] = serde_json::json!(1);
    incomplete
        .as_object_mut()
        .expect("receipt object")
        .remove("hostRole");
    let incomplete =
        serde_json::from_value(incomplete).expect("decode incomplete v1 receipt fixture");
    assert!(!child_registration_receipt_schema_is_current(&incomplete));
}

#[test]
fn canonical_reader_returns_stable_v1_and_types_incomplete_v1_as_stale() {
    let project = tempfile::tempdir().expect("temporary project root");
    let path = project.path().join("child-registration.json");
    std::fs::create_dir_all(path.parent().expect("registration parent"))
        .expect("create registration parent");

    let current = current_registration_receipt_fixture();
    std::fs::write(
        &path,
        serde_json::to_vec(&current).expect("encode current v1 receipt"),
    )
    .expect("write current v1 receipt");
    let reread = read_child_registration_at_path(&path, "project", "root", "asp_explorer")
        .expect("read current v1 receipt")
        .expect("v1 receipt exists");
    assert_eq!(reread.generation, current.generation);
    assert_eq!(reread.schema_version, 1);

    let mut incomplete = current;
    incomplete.host_role.clear();
    std::fs::write(
        &path,
        serde_json::to_vec(&incomplete).expect("encode incomplete v1 receipt"),
    )
    .expect("write incomplete v1 receipt");
    let error = read_child_registration_at_path(&path, "project", "root", "asp_explorer")
        .expect_err("incomplete v1 receipt must be typed stale");
    assert_eq!(
        error,
        "child-registration-receipt-schema-stale-reregister-required"
    );
    assert!(
        read_replaceable_child_registration_at_path(&path, "project", "root", "asp_explorer")
            .expect("stale receipt is replaceable")
            .is_none()
    );
}

fn registration_receipt_fixture_for(
    root_session_id: &str,
    parent_session_id: &str,
    child_session_id: &str,
    generation: u64,
) -> ChildSessionRegistrationReceipt {
    serde_json::from_value(serde_json::json!({
        "schemaId": "agent.child-session-registration",
        "schemaVersion": 1,
        "projectId": "project",
        "rootSessionId": root_session_id,
        "parentSessionId": parent_session_id,
        "childSessionId": child_session_id,
        "hostRole": "explorer",
        "residentId": "asp_explorer",
        "canonicalAgentName": "asp_explorer",
        "platform": "codex",
        "routeKey": "asp_explorer",
        "routeDigest": format!("blake3-256:{}", "a".repeat(64)),
        "profileId": "agents/asp_explorer_codex.toml",
        "profileDigest": format!("blake3-256:{}", "b".repeat(64)),
        "policyDigest": format!("blake3-256:{}", "c".repeat(64)),
        "definitionSchemaId": "urn:agent-semantic-protocols:schema:codex-agent-definition",
        "deniedActions": ["edit"],
        "allowedRuleIntents": ["reasoning-search", "structured-projection"],
        "sandboxMode": "read-only",
        "sessionLifetime": "resident",
        "generation": generation,
        "lifecycleState": "live",
        "routable": true,
        "hostCallReceiptDigest": format!("blake3-256:{}", "d".repeat(64)),
        "registrationAuthority": "project-child-registration-authority"
    }))
    .expect("decode child-identity registration receipt fixture")
}

#[test]
fn choice_plane_projects_the_canonical_receipt_generation_without_a_second_index() {
    let receipt = current_registration_receipt_fixture();
    let (state, generation, reason_kind) =
        child_registration_control_plane_projection(Some(&receipt));
    assert_eq!(state, "registered");
    assert_eq!(generation, receipt.generation);
    assert!(generation > 0);
    assert_eq!(reason_kind, None);

    let (state, generation, reason_kind) = child_registration_control_plane_projection(None);
    assert_eq!(state, "registration-required");
    assert_eq!(generation, 0);
    assert_eq!(reason_kind, Some("host-agent-registration-required"));
}

#[tokio::test]
async fn atomic_publish_is_immediately_readable_by_the_canonical_reader() {
    let directory = tempfile::tempdir().expect("temporary registration directory");
    let path = directory.path().join("child-registration.json");
    let temporary = directory.path().join(".child-registration.tmp");
    let receipt = current_registration_receipt_fixture();

    publish_child_registration_receipt_at_path(&path, &temporary, &receipt)
        .await
        .expect("atomically publish stable v1 receipt");
    assert!(!temporary.exists());
    let reread = read_child_registration_at_path(&path, "project", "root", "asp_explorer")
        .expect("immediate canonical read")
        .expect("published receipt exists");
    assert_eq!(reread.generation, receipt.generation);
    assert_eq!(reread.route_digest, receipt.route_digest);
    assert_eq!(reread.profile_digest, receipt.profile_digest);
    assert_eq!(reread.policy_digest, receipt.policy_digest);
}

#[tokio::test]
async fn child_identity_registry_isolates_two_roots_and_children_sequentially() {
    let project = tempfile::tempdir().expect("temporary project root");
    let first_path = child_registration_path(
        project.path(),
        "project",
        "root-a",
        "parent-a",
        "child-a",
        "asp_explorer",
    )
    .expect("first child registration path");
    let second_path = child_registration_path(
        project.path(),
        "project",
        "root-b",
        "parent-b",
        "child-b",
        "asp_explorer",
    )
    .expect("second child registration path");
    assert_ne!(first_path, second_path);
    let host_sessions = project.path().join("host-sessions");
    let first_path = host_sessions
        .join("root-a-child-a")
        .join("child-registration.json");
    let second_path = host_sessions
        .join("root-b-child-b")
        .join("child-registration.json");
    std::fs::create_dir_all(first_path.parent().expect("first authority directory"))
        .expect("create first authority directory");
    std::fs::create_dir_all(second_path.parent().expect("second authority directory"))
        .expect("create second authority directory");
    let first = registration_receipt_fixture_for("root-a", "parent-a", "child-a", 11);
    let second = registration_receipt_fixture_for("root-b", "parent-b", "child-b", 17);
    publish_child_registration_receipt_at_path(
        &first_path,
        &first_path.with_extension("tmp"),
        &first,
    )
    .await
    .expect("publish first child");
    publish_child_registration_receipt_at_path(
        &second_path,
        &second_path.with_extension("tmp"),
        &second,
    )
    .await
    .expect("publish second child");

    let first_read = read_current_child_registration_at_path(
        &first_path,
        "project",
        "root-a",
        "parent-a",
        "child-a",
        "asp_explorer",
    )
    .expect("read first child")
    .expect("first child exists");
    let second_read = read_current_child_registration_at_path(
        &second_path,
        "project",
        "root-b",
        "parent-b",
        "child-b",
        "asp_explorer",
    )
    .expect("read second child")
    .expect("second child exists");
    assert_eq!(first_read.generation, 11);
    assert_eq!(second_read.generation, 17);
    assert!(child_registration_control_plane_projection(Some(&first_read)).1 > 0);
    assert!(child_registration_control_plane_projection(Some(&second_read)).1 > 0);
    assert!(
        read_current_child_registration_at_path(
            &first_path,
            "project",
            "root-a",
            "parent-b",
            "child-b",
            "asp_explorer",
        )
        .expect_err("wrong binding must fail closed")
        .contains("child-identity-mismatch")
    );
}

#[tokio::test]
async fn concurrent_child_publications_cannot_retire_or_cross_claim() {
    let project = tempfile::tempdir().expect("temporary project root");
    let first_path = child_registration_path(
        project.path(),
        "project",
        "root-a",
        "parent-a",
        "child-a",
        "asp_explorer",
    )
    .expect("first child registration path");
    let second_path = child_registration_path(
        project.path(),
        "project",
        "root-b",
        "parent-b",
        "child-b",
        "asp_explorer",
    )
    .expect("second child registration path");
    assert_ne!(first_path, second_path);
    let host_sessions = project.path().join("host-sessions");
    let first_path = host_sessions
        .join("root-a-child-a")
        .join("child-registration.json");
    let second_path = host_sessions
        .join("root-b-child-b")
        .join("child-registration.json");
    std::fs::create_dir_all(first_path.parent().expect("first authority directory"))
        .expect("create first authority directory");
    std::fs::create_dir_all(second_path.parent().expect("second authority directory"))
        .expect("create second authority directory");
    let first = registration_receipt_fixture_for("root-a", "parent-a", "child-a", 23);
    let second = registration_receipt_fixture_for("root-b", "parent-b", "child-b", 29);
    let first_temporary = first_path.with_extension("tmp");
    let second_temporary = second_path.with_extension("tmp");
    let (first_result, second_result) = tokio::join!(
        publish_child_registration_receipt_at_path(&first_path, &first_temporary, &first,),
        publish_child_registration_receipt_at_path(&second_path, &second_temporary, &second,)
    );
    first_result.expect("publish first child concurrently");
    second_result.expect("publish second child concurrently");

    let root_a = read_current_child_registrations_for_route_at_root(
        &host_sessions,
        "project",
        "root-a",
        "asp_explorer",
    )
    .expect("read root-a registrations");
    let root_b = read_current_child_registrations_for_route_at_root(
        &host_sessions,
        "project",
        "root-b",
        "asp_explorer",
    )
    .expect("read root-b registrations");
    assert_eq!(root_a.len(), 1);
    assert_eq!(root_b.len(), 1);
    assert_eq!(root_a[0].child_session_id, "child-a");
    assert_eq!(root_b[0].child_session_id, "child-b");
    assert!(child_registration_control_plane_projection(root_a.first()).1 > 0);
    assert!(child_registration_control_plane_projection(root_b.first()).1 > 0);
}

#[tokio::test]
async fn incomplete_v1_is_replaceable_only_for_its_matching_child_entry() {
    let directory = tempfile::tempdir().expect("temporary child registry");
    let stale_path = directory
        .path()
        .join("child-a")
        .join("child-registration.json");
    let current_path = directory
        .path()
        .join("child-b")
        .join("child-registration.json");
    std::fs::create_dir_all(stale_path.parent().expect("stale child directory"))
        .expect("create stale child directory");
    std::fs::create_dir_all(current_path.parent().expect("current child directory"))
        .expect("create current child directory");
    let mut stale = registration_receipt_fixture_for("root", "parent-a", "child-a", 31);
    stale.host_role.clear();
    let current = registration_receipt_fixture_for("root", "parent-b", "child-b", 37);
    publish_child_registration_receipt_at_path(
        &stale_path,
        &stale_path.with_extension("tmp"),
        &stale,
    )
    .await
    .expect("publish stale matching entry");
    publish_child_registration_receipt_at_path(
        &current_path,
        &current_path.with_extension("tmp"),
        &current,
    )
    .await
    .expect("publish independent current entry");

    assert!(read_replaceable_child_registration_at_path(
        &stale_path,
        "project",
        "root",
        "asp_explorer",
    )
    .expect("matching stale entry is replaceable")
    .is_none());
    let current_read = read_current_child_registration_at_path(
        &current_path,
        "project",
        "root",
        "parent-b",
        "child-b",
        "asp_explorer",
    )
    .expect("independent current entry remains readable")
    .expect("independent current entry exists");
    assert_eq!(current_read.generation, 37);
}

#[test]
fn cross_process_canonical_reader_child() {
    let Some(path) = std::env::var_os("ASP_TEST_CHILD_REGISTRATION_PATH") else {
        return;
    };
    let receipt = read_child_registration_at_path(
        std::path::Path::new(&path),
        "project",
        "root",
        "asp_explorer",
    )
    .expect("cross-process canonical read")
    .expect("cross-process receipt exists");
    assert_eq!(receipt.schema_version, 1);
    assert!(receipt.generation > 0);
}

#[tokio::test]
async fn atomic_publication_is_readable_from_a_fresh_process() {
    let directory = tempfile::tempdir().expect("temporary registration directory");
    let path = directory.path().join("child-registration.json");
    let temporary = directory.path().join(".child-registration.tmp");
    let receipt = current_registration_receipt_fixture();
    publish_child_registration_receipt_at_path(&path, &temporary, &receipt)
        .await
        .expect("atomically publish stable v1 receipt");

    let status = std::process::Command::new(std::env::current_exe().expect("current test binary"))
        .arg("cross_process_canonical_reader_child")
        .arg("--nocapture")
        .env("ASP_TEST_CHILD_REGISTRATION_PATH", &path)
        .status()
        .expect("launch cross-process canonical reader");
    assert!(status.success());
}

#[tokio::test]
async fn incomplete_v1_receipt_materializes_current_v1_and_continues_the_same_pretool_call() {
    let current = current_registration_receipt_fixture();
    let mut stale = current.clone();
    stale.schema_id = "agent.child-session-registration".to_owned();
    stale.host_role.clear();
    let publishes = std::cell::Cell::new(0);

    let receipt = read_or_materialize_registration_handoff(
        || Ok(Some(stale.clone())),
        child_registration_receipt_schema_is_current,
        || async {
            publishes.set(publishes.get() + 1);
            Ok::<_, String>(current.clone())
        },
    )
    .await
    .expect("stale v1 receipt must be replaced in the same PreTool call");

    assert!(child_registration_receipt_schema_is_current(&receipt));
    assert_eq!(receipt.schema_id, "agent.child-session-registration");
    assert_eq!(receipt.schema_version, 1);
    assert_eq!(receipt.generation, 1);
    assert_eq!(receipt.root_session_id, "root");
    assert_eq!(receipt.parent_session_id, "parent");
    assert_eq!(receipt.child_session_id, "child");
    assert_eq!(receipt.host_role, "explorer");
    assert_eq!(receipt.canonical_agent_name, "asp_explorer");
    assert_eq!(receipt.platform, "codex");
    assert!(receipt.route_digest.starts_with("blake3-256:"));
    assert!(receipt.profile_digest.starts_with("blake3-256:"));
    assert!(receipt.policy_digest.starts_with("blake3-256:"));
    assert_eq!(publishes.get(), 1);
}

#[test]
fn child_registration_uses_the_global_agent_catalog_across_workspaces() {
    let global_state_home = tempfile::tempdir().expect("global State Home");
    let project_state_home = tempfile::tempdir().expect("project Hook state");
    let agents_root = global_state_home.path().join("agents");
    std::fs::create_dir_all(&agents_root).expect("create global agents root");
    for asset in agent_semantic_config::embedded_agent_assets::embedded_agent_assets() {
        std::fs::write(agents_root.join(asset.file_name), asset.contents)
            .expect("materialize embedded agent asset");
    }
    std::fs::create_dir_all(project_state_home.path().join("agents"))
        .expect("create stale project agents root");
    std::fs::write(
        project_state_home.path().join("agents/config.toml"),
        "not = [valid",
    )
    .expect("write stale project catalog");

    let registry_path =
        crate::multi_agent_session::registration::canonical_agent_route_registry_path(
            global_state_home.path(),
        );
    assert_eq!(registry_path, agents_root.join("config.toml"));
    assert_ne!(
        registry_path,
        project_state_home.path().join("agents/config.toml")
    );
    let registry =
        agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform(
            &registry_path,
            "codex",
        )
        .expect("load global agent catalog");
    let route = registry
        .compile_route_for_platform_host_identity("codex", "explorer")
        .expect("resolve explorer role")
        .expect("explorer route");
    assert_eq!(route.route_key.as_str(), "asp_explorer");
}

#[test]
fn missing_or_corrupt_global_agent_catalog_fails_closed_without_project_fallback() {
    let global_state_home = tempfile::tempdir().expect("global State Home");
    let registry_path =
        crate::multi_agent_session::registration::canonical_agent_route_registry_path(
            global_state_home.path(),
        );
    assert!(
        agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform(
            &registry_path,
            "codex",
        )
        .is_err()
    );

    std::fs::create_dir_all(registry_path.parent().expect("agents root"))
        .expect("create agents root");
    std::fs::write(&registry_path, "not = [valid").expect("write corrupt catalog");
    assert!(
        agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform(
            &registry_path,
            "codex",
        )
        .is_err()
    );
}

#[tokio::test]
async fn stale_schema_is_atomically_replaced_and_the_first_tool_call_continues() {
    let publishes = std::cell::Cell::new(0);
    let receipt = read_or_materialize_registration_handoff(
        || Ok(Some("schema-v1-stale")),
        |receipt| *receipt == "schema-v1-current",
        || async {
            publishes.set(publishes.get() + 1);
            Ok::<&str, String>("schema-v1-current")
        },
    )
    .await
    .expect("stale receipt replacement must continue the same first tool call");

    assert_eq!(receipt, "schema-v1-current");
    assert_eq!(publishes.get(), 1);
}

#[tokio::test]
async fn first_child_tool_materializes_a_missing_registration_and_continues() {
    let publishes = std::cell::Cell::new(0);
    let receipt = read_or_materialize_registration_handoff(
        || Ok::<Option<&str>, String>(None),
        |_| true,
        || async {
            publishes.set(publishes.get() + 1);
            Ok("current-child")
        },
    )
    .await
    .expect("first child tool registration");

    assert_eq!(receipt, "current-child");
    assert_eq!(publishes.get(), 1);
}

#[tokio::test]
async fn current_child_registration_is_idempotent_and_skips_publication() {
    let publishes = std::cell::Cell::new(0);
    let receipt = read_or_materialize_registration_handoff(
        || Ok::<_, String>(Some("current-child")),
        |value| *value == "current-child",
        || async {
            publishes.set(publishes.get() + 1);
            Ok("replacement-child")
        },
    )
    .await
    .expect("idempotent child registration");

    assert_eq!(receipt, "current-child");
    assert_eq!(publishes.get(), 0);
}

#[tokio::test]
async fn stale_child_registration_cannot_cross_claim_the_first_tool() {
    let publishes = std::cell::Cell::new(0);
    let receipt = read_or_materialize_registration_handoff(
        || Ok::<_, String>(Some("previous-child")),
        |value| *value == "current-child",
        || async {
            publishes.set(publishes.get() + 1);
            Ok("current-child")
        },
    )
    .await
    .expect("current child registration");

    assert_eq!(receipt, "current-child");
    assert_eq!(publishes.get(), 1);
}
