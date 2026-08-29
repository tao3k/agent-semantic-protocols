use std::path::Path;

use agent_semantic_config::{
    CODEX_PLUGIN_HOOKS_RELATIVE_PATH, CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH,
    CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, CodexPluginPayloadState, inspect_codex_plugin_payload,
    load_codex_plugin_payload_identity,
};

const SOURCE_MANIFEST: &[u8] =
    include_bytes!("../../../asp-codex-plugin/.codex-plugin/plugin.json");
const SOURCE_HOOKS: &[u8] = include_bytes!("../../../asp-codex-plugin/hooks/hooks.json");
const SOURCE_LAUNCHER: &[u8] = include_bytes!("../../../asp-codex-plugin/bin/asp-hook");

#[test]
fn canonical_plugin_payload_and_installed_cache_are_content_identical() {
    let fixture = tempfile::tempdir().expect("plugin payload fixture");
    let source = fixture.path().join("source");
    let installed = fixture.path().join("installed");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    write_bundle(&installed, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);

    let inspection = inspect_codex_plugin_payload(&source, &installed).expect("inspect current");
    assert_eq!(inspection.state, CodexPluginPayloadState::Current);
    assert_eq!(inspection.installed.as_ref(), Some(&inspection.source));
}

#[test]
fn hooks_or_launcher_drift_requires_explicit_plugin_publication() {
    let fixture = tempfile::tempdir().expect("plugin drift fixture");
    let source = fixture.path().join("source");
    let installed = fixture.path().join("installed");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    write_bundle(&installed, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);

    let mut hooks = SOURCE_HOOKS.to_vec();
    hooks.extend_from_slice(b"\n");
    std::fs::write(source.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH), hooks)
        .expect("mutate source hooks");
    assert_eq!(
        inspect_codex_plugin_payload(&source, &installed)
            .expect("inspect hooks drift")
            .state,
        CodexPluginPayloadState::PublicationRequired,
    );

    std::fs::write(source.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH), SOURCE_HOOKS)
        .expect("restore hooks");
    let mut launcher = SOURCE_LAUNCHER.to_vec();
    launcher.extend_from_slice(b"\n");
    std::fs::write(source.join(CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH), launcher)
        .expect("mutate source launcher");
    assert_eq!(
        inspect_codex_plugin_payload(&source, &installed)
            .expect("inspect launcher drift")
            .state,
        CodexPluginPayloadState::PublicationRequired,
    );
}

#[test]
fn missing_or_corrupt_installed_cache_is_typed_and_fail_closed() {
    let fixture = tempfile::tempdir().expect("plugin corrupt fixture");
    let source = fixture.path().join("source");
    let installed = fixture.path().join("installed");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);

    assert_eq!(
        inspect_codex_plugin_payload(&source, &installed)
            .expect("inspect missing cache")
            .state,
        CodexPluginPayloadState::Missing,
    );
    write_bundle(&installed, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    std::fs::write(installed.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH), b"{")
        .expect("corrupt installed hooks");
    let inspection = inspect_codex_plugin_payload(&source, &installed).expect("inspect corrupt");
    assert_eq!(inspection.state, CodexPluginPayloadState::Corrupt);
    assert!(inspection.detail.is_some());
}

#[test]
fn payload_identity_binds_manifest_hooks_and_launcher_without_runtime_state() {
    let fixture = tempfile::tempdir().expect("plugin identity fixture");
    let source = fixture.path().join("source");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    let identity = load_codex_plugin_payload_identity(&source).expect("payload identity");
    assert_eq!(identity.plugin_name, "asp-codex-plugin");
    assert!(identity.version.starts_with("0.1.0+codex."));
    assert!(identity.digest.starts_with("blake3-256:"));
    assert!(!source.join("runtime").exists());
    assert!(!source.join("hooks/current").exists());
}

#[test]
fn payload_validation_rejects_manifest_declared_hook_or_skill_facades() {
    let fixture = tempfile::tempdir().expect("plugin manifest fixture");
    let source = fixture.path().join("source");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    let manifest_path = source.join(CODEX_PLUGIN_MANIFEST_RELATIVE_PATH);
    let mut manifest =
        serde_json::from_slice::<serde_json::Value>(SOURCE_MANIFEST).expect("parse manifest");
    manifest["hooks"] = serde_json::json!("hooks/hooks.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
    )
    .expect("write invalid manifest");
    assert!(
        load_codex_plugin_payload_identity(&source)
            .expect_err("manifest-owned hooks must fail")
            .contains("Hook-only standard-directory manifest")
    );
}

#[test]
fn payload_validation_rejects_hook_handlers_that_bypass_the_plugin_launcher() {
    let fixture = tempfile::tempdir().expect("plugin handler fixture");
    let source = fixture.path().join("source");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    let hooks_path = source.join(CODEX_PLUGIN_HOOKS_RELATIVE_PATH);
    let mut hooks = serde_json::from_slice::<serde_json::Value>(SOURCE_HOOKS).expect("parse hooks");
    let events = hooks["hooks"].as_object_mut().expect("events");
    let first_group = events
        .values_mut()
        .next()
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|groups| groups.first_mut())
        .expect("first hook group");
    first_group["hooks"][0]["command"] = serde_json::json!("asp hook pre-tool");
    std::fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("encode hooks"),
    )
    .expect("write bypassing handler");
    assert!(
        load_codex_plugin_payload_identity(&source)
            .expect_err("launcher bypass must fail")
            .contains("bypasses the plugin-owned launcher")
    );
}

#[test]
fn payload_validation_rejects_runtime_coupled_launcher() {
    let fixture = tempfile::tempdir().expect("plugin launcher fixture");
    let source = fixture.path().join("source");
    write_bundle(&source, SOURCE_MANIFEST, SOURCE_HOOKS, SOURCE_LAUNCHER);
    let launcher_path = source.join(CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH);
    let launcher = String::from_utf8(SOURCE_LAUNCHER.to_vec())
        .expect("launcher UTF-8")
        .replace("hooks/current", "runtime/bin/asp");
    std::fs::write(&launcher_path, launcher).expect("write Runtime-coupled launcher");
    assert!(
        load_codex_plugin_payload_identity(&source)
            .expect_err("Runtime-coupled launcher must fail")
            .contains("immutable HookGeneration")
    );
}

fn write_bundle(root: &Path, manifest: &[u8], hooks: &[u8], launcher: &[u8]) {
    for (relative, bytes) in [
        (CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, manifest),
        (CODEX_PLUGIN_HOOKS_RELATIVE_PATH, hooks),
        (CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH, launcher),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("payload parent"))
            .expect("create payload parent");
        std::fs::write(path, bytes).expect("write payload file");
    }
}
