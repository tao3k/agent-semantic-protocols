use super::{LaunchdReconcilePlan, canonical_supervisor_runtime_artifact, launchd_reconcile_plan};

#[tokio::test(flavor = "current_thread")]
async fn supervisor_definition_pins_the_digest_addressed_runtime_artifact() {
    use std::os::unix::fs::symlink;

    let protocol_home = std::env::temp_dir().join(format!(
        "asp-runtime-supervisor-artifact-{}",
        std::process::id()
    ));
    let digest = "a".repeat(64);
    let artifact = protocol_home
        .join("runtime")
        .join("artifacts")
        .join("blake3-256")
        .join(&digest)
        .join("asp");
    std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
        .expect("create digest-addressed artifact directory");
    std::fs::write(&artifact, b"fixture").expect("write digest-addressed artifact");
    let stable_entry = protocol_home.join("runtime").join("bin").join("asp");
    std::fs::create_dir_all(stable_entry.parent().expect("stable entry parent"))
        .expect("create stable runtime directory");
    symlink(&artifact, &stable_entry).expect("publish stable runtime entry");

    let canonical_artifact =
        std::fs::canonicalize(&artifact).expect("canonicalize digest-addressed artifact");
    assert_eq!(
        canonical_supervisor_runtime_artifact(&protocol_home)
            .await
            .expect("resolve supervisor runtime artifact"),
        canonical_artifact
    );

    std::fs::remove_dir_all(protocol_home).expect("remove supervisor artifact fixture");
}

#[test]
fn launchd_definition_does_not_inherit_the_ten_second_spawn_throttle() {
    let definition = include_str!(
        "../../templates/server/dev.tao3k.agent-semantic-protocols.asp-runtime-server.plist"
    );
    assert!(
        definition.contains("<key>ThrottleInterval</key>\n  <integer>1</integer>"),
        "atomic runtime replacement must not inherit launchd's ten-second default throttle"
    );
}

#[test]
fn unchanged_loaded_service_uses_atomic_kickstart() {
    let plan = launchd_reconcile_plan(true, false);
    assert_eq!(plan, LaunchdReconcilePlan::Kickstart);
    assert!(plan.requires_kickstart());
}

#[test]
fn missing_service_bootstraps_without_restarting_the_new_process() {
    let plan = launchd_reconcile_plan(false, false);
    assert_eq!(plan, LaunchdReconcilePlan::Bootstrap);
    assert!(!plan.requires_kickstart());
}

#[test]
fn changed_loaded_definition_rebootstraps_without_restarting_the_new_process() {
    let plan = launchd_reconcile_plan(true, true);
    assert_eq!(plan, LaunchdReconcilePlan::Rebootstrap);
    assert!(!plan.requires_kickstart());
}
