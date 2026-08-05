#[path = "../../src/server/runtime_server_service_catalog.rs"]
mod runtime_server_service_catalog;

#[test]
fn global_runtime_has_one_active_daemon_and_retires_the_legacy_resident() {
    let catalog = runtime_server_service_catalog::runtime_server_service_catalog();
    assert_eq!(
        catalog.active_macos_label,
        "dev.tao3k.agent-semantic-protocols.asp-runtime-server"
    );
    assert_eq!(
        catalog.retired_macos_labels,
        &["dev.tao3k.agent-semantic-protocols.asp-resident"]
    );
    assert!(
        !catalog
            .retired_macos_labels
            .contains(&catalog.active_macos_label)
    );
}
