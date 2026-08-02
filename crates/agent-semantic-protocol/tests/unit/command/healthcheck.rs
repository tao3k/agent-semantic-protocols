use super::{
    CatalogReadinessReceipt, collect_catalog_readiness, healthcheck_command_result, overall_status,
};

#[test]
fn catalog_readiness_success_preserves_read_receipt_without_issue() {
    let mut issues = Vec::new();
    let receipt = collect_catalog_readiness(
        &mut issues,
        Ok(
            super::super::global_provider_catalog::GlobalProviderCatalogReadiness {
                catalog_generation: "generation-test".to_owned(),
                provider_count: 7,
                elapsed_micros: 41,
            },
        ),
    );

    assert!(issues.is_empty());
    assert!(matches!(
        receipt,
        CatalogReadinessReceipt::Ready {
            catalog_generation,
            provider_count: 7,
            elapsed_micros: 41,
        } if catalog_generation == "generation-test"
    ));
}

#[test]
fn catalog_readiness_failure_adds_exact_error_issue() {
    let mut issues = Vec::new();
    let message = "content store publish: Operation not permitted".to_owned();
    let receipt = collect_catalog_readiness(&mut issues, Err(message.clone()));

    assert!(matches!(
        receipt,
        CatalogReadinessReceipt::Error { message: actual } if actual == message
    ));
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, "error");
    assert_eq!(issues[0].code, "global-provider-catalog-readiness-failed");
    assert_eq!(issues[0].message, message);
    assert_eq!(overall_status(&issues), "error");
}

#[test]
fn healthcheck_error_status_returns_nonzero_contract() {
    assert_eq!(
        healthcheck_command_result("error"),
        Err("healthcheck status=error".to_owned())
    );
    assert_eq!(healthcheck_command_result("degraded"), Ok(()));
    assert_eq!(healthcheck_command_result("ok"), Ok(()));
}

#[test]
fn healthcheck_schedules_workspace_generation_without_waiting_for_terminal_state() {
    let source = include_str!("../../../src/command/healthcheck.rs");

    assert!(source.contains("runtime_server_workspace_session_for_admission_async"));
    assert!(source.contains("session.ensure_runtime_generation().await"));
    assert!(source.contains("workspace_generation_started.elapsed().as_micros()"));
    assert!(source.contains("elapsedMicros={} error={}"));
    assert!(!source.contains("wait_terminal"));
}
